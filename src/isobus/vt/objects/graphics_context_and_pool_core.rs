impl ColourMapBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if !matches!(self.entries.len(), 2 | 16 | 256) {
            return Err(Error::invalid_data(
                "Colour Map entry count must be one of the standard sizes: 2, 16, or 256",
            ));
        }
        let mut data = Vec::with_capacity(2 + self.entries.len());
        push_u16_le(&mut data, self.entries.len() as u16);
        data.extend_from_slice(&self.entries);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 2 {
            return Err(Error::invalid_data("Colour Map body too short"));
        }
        let count = u16_le(body) as usize;
        if !matches!(count, 2 | 16 | 256) {
            return Err(Error::invalid_data(
                "Colour Map entry count is not a standard size",
            ));
        }
        if body.len() != 2 + count {
            return Err(Error::invalid_data(
                "Colour Map entry count does not match body length",
            ));
        }
        Ok(Self {
            entries: body[2..].to_vec(),
        })
    }
}

/// Graphics Context body (Type 36).
///
/// ISO 11783-6 Table B.59 defines this as a fixed record carrying viewport,
/// cursor, colour, attribute-reference, canvas-format, option, and transparency
/// colour state. The Graphics Context command stream can change many of these
/// values at runtime, but the pool body supplies the initial drawing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicContextBody {
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub viewport_x: i16,
    pub viewport_y: i16,
    pub canvas_width: u16,
    pub canvas_height: u16,
    /// Raw IEEE-754 `f32` viewport magnification. Defaults to `1.0`.
    pub viewport_zoom_raw: u32,
    pub cursor_x: i16,
    pub cursor_y: i16,
    /// Foreground colour used when options bit 1 is clear.
    pub foreground_colour: u8,
    /// Background colour used when options bit 1 is clear; it also initializes
    /// opaque Graphics Context canvas pixels.
    pub background_colour: u8,
    pub font_attributes: ObjectID,
    pub line_attributes: ObjectID,
    pub fill_attributes: ObjectID,
    /// 0 = monochrome, 1 = 4-bit colour, 2 = 8-bit colour.
    pub format: u8,
    /// Bit 0 = transparent background; bit 1 = draw colours from attribute
    /// objects instead of this object's foreground/background colours.
    pub options: u8,
    /// Palette index treated as transparent when options bit 0 is set.
    pub transparency_colour: u8,
}

impl Default for GraphicContextBody {
    fn default() -> Self {
        Self {
            viewport_width: 0,
            viewport_height: 0,
            viewport_x: 0,
            viewport_y: 0,
            canvas_width: 0,
            canvas_height: 0,
            viewport_zoom_raw: 1.0f32.to_bits(),
            cursor_x: 0,
            cursor_y: 0,
            foreground_colour: 1,
            background_colour: 0,
            font_attributes: ObjectID::NULL,
            line_attributes: ObjectID::NULL,
            fill_attributes: ObjectID::NULL,
            format: 2,
            options: 0,
            transparency_colour: 0,
        }
    }
}

impl GraphicContextBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.viewport_width > 32767
            || self.viewport_height > 32767
            || self.canvas_width > 32767
            || self.canvas_height > 32767
        {
            return Err(Error::invalid_data(
                "Graphic Context dimensions must be in 0..=32767",
            ));
        }
        if !graphic_context_zoom_raw_is_valid(self.viewport_zoom_raw) {
            return Err(Error::invalid_data(
                "Graphic Context viewport zoom must be finite and in -32.0..=32.0",
            ));
        }
        if self.format > 2 {
            return Err(Error::invalid_data(
                "Graphic Context format must be in 0..=2",
            ));
        }
        if self.options & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Graphic Context options contain reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(31);
        push_u16_le(&mut data, self.viewport_width);
        push_u16_le(&mut data, self.viewport_height);
        push_u16_le(&mut data, self.viewport_x as u16);
        push_u16_le(&mut data, self.viewport_y as u16);
        push_u16_le(&mut data, self.canvas_width);
        push_u16_le(&mut data, self.canvas_height);
        data.extend_from_slice(&self.viewport_zoom_raw.to_le_bytes());
        push_u16_le(&mut data, self.cursor_x as u16);
        push_u16_le(&mut data, self.cursor_y as u16);
        data.push(self.foreground_colour);
        data.push(self.background_colour);
        push_u16_le(&mut data, self.font_attributes);
        push_u16_le(&mut data, self.line_attributes);
        push_u16_le(&mut data, self.fill_attributes);
        data.push(self.format);
        data.push(self.options);
        data.push(self.transparency_colour);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 31 {
            return Err(Error::invalid_data("Graphic Context body too short"));
        }
        if body[28] > 2 {
            return Err(Error::invalid_data(
                "Graphic Context body contains reserved format",
            ));
        }
        if body[29] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Graphic Context body contains reserved option bits",
            ));
        }
        let viewport_zoom_raw = u32::from_le_bytes([body[12], body[13], body[14], body[15]]);
        if !graphic_context_zoom_raw_is_valid(viewport_zoom_raw) {
            return Err(Error::invalid_data(
                "Graphic Context body contains invalid viewport zoom",
            ));
        }
        let viewport_width = u16_le(&body[0..]);
        let viewport_height = u16_le(&body[2..]);
        let canvas_width = u16_le(&body[8..]);
        let canvas_height = u16_le(&body[10..]);
        if viewport_width > 32767
            || viewport_height > 32767
            || canvas_width > 32767
            || canvas_height > 32767
        {
            return Err(Error::invalid_data(
                "Graphic Context body contains out-of-range dimensions",
            ));
        }
        Ok(Self {
            viewport_width,
            viewport_height,
            viewport_x: u16_le(&body[4..]) as i16,
            viewport_y: u16_le(&body[6..]) as i16,
            canvas_width,
            canvas_height,
            viewport_zoom_raw,
            cursor_x: u16_le(&body[16..]) as i16,
            cursor_y: u16_le(&body[18..]) as i16,
            foreground_colour: body[20],
            background_colour: body[21],
            font_attributes: u16_le_as(&body[22..]),
            line_attributes: u16_le_as(&body[24..]),
            fill_attributes: u16_le_as(&body[26..]),
            format: body[28],
            options: body[29],
            transparency_colour: body[30],
        })
    }
}

fn graphic_context_zoom_raw_is_valid(raw: u32) -> bool {
    let zoom = f32::from_bits(raw);
    zoom.is_finite() && (-32.0..=32.0).contains(&zoom)
}

// ─── VTObject ─────────────────────────────────────────────────────────

/// One VT object as it appears in the pool.
///
/// Wire layout (ISO 11783-6 object pool — no per-object length prefix):
/// ```text
/// [0..1] Object ID (LE)
/// [2]    Object type
/// [3..]  Object-specific body (for parent objects the child count + child
///        records + macro count + macro records form the body tail, per
///        §4.6.x). Object boundaries are recovered by `object_body_total_len`.
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VTObject {
    pub id: ObjectID,
    pub r#type: ObjectType,
    pub body: Vec<u8>,
    pub children: Vec<ObjectID>,
    /// Positional child references as encoded by ISO 11783-6 (object id
    /// plus signed X/Y relative to this object). Populated by the
    /// standard codec; the legacy `children` list is kept as a derived,
    /// position-less view for backwards compatibility.
    pub children_pos: Vec<ChildRef>,
    /// Macro references (event id + macro id) that follow the child list
    /// in parent objects per ISO 11783-6.
    pub macros: Vec<MacroRef>,
}

impl VTObject {
    #[must_use]
    pub fn with_id(mut self, v: impl Into<ObjectID>) -> Self {
        self.id = v.into();
        self
    }

    #[must_use]
    pub fn with_type(mut self, v: ObjectType) -> Self {
        self.r#type = v;
        self
    }

    #[must_use]
    pub fn with_body(mut self, v: Vec<u8>) -> Self {
        self.body = v;
        self
    }

    /// Replace the children list. Accepts any iterator whose items
    /// can convert into an [`ObjectID`] (so `vec![1u16, 2u16]` and
    /// `vec![ObjectID(1), ObjectID(2)]` both work).
    ///
    /// This updates the legacy position-less [`children`](Self::children)
    /// list and also seeds [`children_pos`](Self::children_pos) at the
    /// origin so both views stay consistent.
    #[must_use]
    pub fn with_children<I, T>(mut self, v: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ObjectID>,
    {
        let ids: Vec<ObjectID> = v.into_iter().map(Into::into).collect();
        self.children_pos = ids.iter().map(|&id| ChildRef::at_origin(id)).collect();
        self.children = ids;
        self
    }

    /// Replace the positional child list (object id + signed X/Y). Also
    /// refreshes the legacy [`children`](Self::children) view.
    #[must_use]
    pub fn with_children_pos<I>(mut self, v: I) -> Self
    where
        I: IntoIterator<Item = ChildRef>,
    {
        let refs: Vec<ChildRef> = v.into_iter().collect();
        self.children = refs.iter().map(|r| r.id).collect();
        self.children_pos = refs;
        self
    }

    pub fn add_child(&mut self, v: impl Into<ObjectID>) -> &mut Self {
        let id = v.into();
        self.children.push(id);
        self.children_pos.push(ChildRef::at_origin(id));
        self
    }

    /// Add a positional child (object id + signed X/Y).
    pub fn add_child_pos(&mut self, id: impl Into<ObjectID>, x: i16, y: i16) -> &mut Self {
        let id = id.into();
        self.children.push(id);
        self.children_pos.push(ChildRef::new(id, x, y));
        self
    }

    /// Add a macro reference (event id + macro id).
    pub fn add_macro(&mut self, event_id: u8, macro_id: u8) -> &mut Self {
        self.macros.push(MacroRef::new(event_id, macro_id));
        self
    }

    // ─── Type-specific body helpers ───────────────────────────────────

    pub fn with_window_mask_body(mut self, wm: &WindowMaskBody) -> Result<Self> {
        self.body = wm.encode()?;
        Ok(self)
    }

    #[must_use]
    pub fn with_key_group_body(mut self, kg: &KeyGroupBody) -> Self {
        self.body = kg.encode();
        self
    }

    #[must_use]
    pub fn with_key_body(mut self, k: &KeyBody) -> Self {
        self.body = k.encode();
        self
    }

    #[must_use]
    pub fn with_macro_body(mut self, m: &MacroBody) -> Self {
        self.body = m.encode();
        self
    }

    pub fn with_alarm_mask_body(mut self, am: &AlarmMaskBody) -> Result<Self> {
        self.body = am.encode()?;
        Ok(self)
    }

    #[must_use]
    pub fn with_data_mask_body(mut self, dm: &DataMaskBody) -> Self {
        self.body = dm.encode();
        self
    }

    #[must_use]
    pub fn with_container_body(mut self, c: &ContainerBody) -> Self {
        self.body = c.encode();
        self
    }

    #[must_use]
    pub fn with_soft_key_mask_body(mut self, skm: &SoftKeyMaskBody) -> Self {
        self.body = skm.encode().to_vec();
        self
    }

    #[must_use]
    pub fn with_button_body(mut self, b: &ButtonBody) -> Self {
        self.body = b.encode();
        self
    }

    #[must_use]
    pub fn with_number_variable_body(mut self, n: &NumberVariableBody) -> Self {
        self.body = n.encode();
        self
    }

    #[must_use]
    pub fn with_string_variable_body(mut self, s: &StringVariableBody) -> Self {
        self.body = s.encode();
        self
    }

    #[must_use]
    pub fn with_font_attributes_body(mut self, f: &FontAttributesBody) -> Self {
        self.body = f.encode();
        self
    }

    #[must_use]
    pub fn with_line_attributes_body(mut self, l: &LineAttributesBody) -> Self {
        self.body = l.encode();
        self
    }

    pub fn with_fill_attributes_body(mut self, f: &FillAttributesBody) -> Result<Self> {
        self.body = f.encode()?;
        Ok(self)
    }

    pub fn with_input_attributes_body(mut self, i: &InputAttributesBody) -> Result<Self> {
        self.body = i.encode()?;
        Ok(self)
    }

    pub fn with_extended_input_attributes_body(
        mut self,
        i: &ExtendedInputAttributesBody,
    ) -> Result<Self> {
        self.body = i.encode()?;
        Ok(self)
    }

    #[must_use]
    pub fn with_object_pointer_body(mut self, p: &ObjectPointerBody) -> Self {
        self.body = p.encode();
        self
    }

    pub fn with_output_line_body(mut self, l: &OutputLineBody) -> Result<Self> {
        self.body = l.encode()?;
        Ok(self)
    }

    pub fn with_output_rectangle_body(mut self, r: &OutputRectangleBody) -> Result<Self> {
        self.body = r.encode()?;
        Ok(self)
    }

    pub fn with_output_ellipse_body(mut self, e: &OutputEllipseBody) -> Result<Self> {
        self.body = e.encode()?;
        Ok(self)
    }

    pub fn with_output_polygon_body(mut self, p: &OutputPolygonBody) -> Result<Self> {
        self.body = p.encode()?;
        Ok(self)
    }

    pub fn with_input_boolean_body(mut self, b: &InputBooleanBody) -> Result<Self> {
        self.body = b.encode()?;
        Ok(self)
    }

    pub fn with_input_string_body(mut self, s: &InputStringBody) -> Result<Self> {
        self.body = s.encode()?;
        Ok(self)
    }

    pub fn with_input_number_body(mut self, n: &InputNumberBody) -> Result<Self> {
        self.body = n.encode()?;
        Ok(self)
    }

    pub fn with_input_list_body(mut self, l: &InputListBody) -> Result<Self> {
        self.body = l.encode()?;
        Ok(self)
    }

    pub fn with_output_list_body(mut self, l: &OutputListBody) -> Result<Self> {
        self.body = l.encode()?;
        Ok(self)
    }

    pub fn with_output_string_body(mut self, s: &OutputStringBody) -> Result<Self> {
        self.body = s.encode()?;
        Ok(self)
    }

    pub fn with_output_number_body(mut self, n: &OutputNumberBody) -> Result<Self> {
        self.body = n.encode()?;
        Ok(self)
    }

    pub fn with_meter_body(mut self, m: &MeterBody) -> Result<Self> {
        self.body = m.encode()?;
        Ok(self)
    }

    pub fn with_linear_bar_graph_body(mut self, b: &LinearBarGraphBody) -> Result<Self> {
        self.body = b.encode()?;
        Ok(self)
    }

    pub fn with_arched_bar_graph_body(mut self, b: &ArchedBarGraphBody) -> Result<Self> {
        self.body = b.encode()?;
        Ok(self)
    }

    pub fn with_picture_graphic_body(mut self, p: &PictureGraphicBody) -> Result<Self> {
        self.body = p.encode()?;
        Ok(self)
    }

    #[must_use]
    pub fn with_working_set_body(mut self, w: &WorkingSetBody) -> Self {
        self.body = w.encode().to_vec();
        self
    }

    pub fn with_aux_function_body(mut self, a: &AuxFunctionBody) -> Result<Self> {
        self.body = a.encode()?;
        Ok(self)
    }

    pub fn with_aux_input_body(mut self, a: &AuxInputBody) -> Result<Self> {
        self.body = a.encode()?;
        Ok(self)
    }

    pub fn with_aux_function2_body(mut self, a: &AuxFunction2Body) -> Result<Self> {
        self.body = a.encode()?;
        Ok(self)
    }

    pub fn with_aux_input2_body(mut self, a: &AuxInput2Body) -> Result<Self> {
        self.body = a.encode()?;
        Ok(self)
    }

    pub fn with_aux_control_designator_body(
        mut self,
        a: &AuxControlDesignatorBody,
    ) -> Result<Self> {
        self.body = a.encode()?;
        Ok(self)
    }

    pub fn with_graphic_data_body(mut self, g: &GraphicDataBody) -> Result<Self> {
        self.body = g.encode()?;
        Ok(self)
    }

    pub fn with_scaled_graphic_body(mut self, g: &ScaledGraphicBody) -> Result<Self> {
        self.body = g.encode()?;
        Ok(self)
    }

    pub fn with_scaled_bitmap_body(mut self, b: &ScaledBitmapBody) -> Result<Self> {
        self.body = b.encode()?;
        Ok(self)
    }

    pub fn with_animation_body(mut self, a: &AnimationBody) -> Result<Self> {
        self.body = a.encode()?;
        Ok(self)
    }

    pub fn with_colour_map_body(mut self, c: &ColourMapBody) -> Result<Self> {
        self.body = c.encode()?;
        Ok(self)
    }

    pub fn with_graphic_context_body(mut self, g: &GraphicContextBody) -> Result<Self> {
        self.body = g.encode()?;
        Ok(self)
    }

    pub fn with_external_object_definition_body(
        mut self,
        e: &ExternalObjectDefinitionBody,
    ) -> Result<Self> {
        self.body = e.encode()?;
        Ok(self)
    }

    pub fn with_external_reference_name_body(mut self, e: &ExternalReferenceNameBody) -> Self {
        self.body = e.encode();
        self
    }

    #[must_use]
    pub fn with_external_object_pointer_body(mut self, e: &ExternalObjectPointerBody) -> Self {
        self.body = e.encode();
        self
    }

    pub fn with_colour_palette_body(mut self, c: &ColourPaletteBody) -> Result<Self> {
        self.body = c.encode()?;
        Ok(self)
    }

    pub fn with_working_set_special_controls_body(
        mut self,
        c: &WorkingSetSpecialControlsBody,
    ) -> Result<Self> {
        self.body = c.encode()?;
        Ok(self)
    }

    #[must_use]
    pub fn with_graphics_context_body(mut self, g: &GraphicsContextBody) -> Self {
        self.body = g.encode();
        self
    }

    #[must_use]
    pub fn with_object_label_ref_body(mut self, o: &ObjectLabelRefBody) -> Self {
        self.body = o.encode();
        self
    }

    pub fn get_window_mask_body(&self) -> Result<WindowMaskBody> {
        if self.r#type != ObjectType::WindowMask {
            return Err(Error::invalid_data("object is not a Window Mask"));
        }
        WindowMaskBody::decode(&self.body)
    }

    pub fn get_key_group_body(&self) -> Result<KeyGroupBody> {
        if self.r#type != ObjectType::KeyGroup {
            return Err(Error::invalid_data("object is not a Key Group"));
        }
        KeyGroupBody::decode(&self.body)
    }

    pub fn get_key_body(&self) -> Result<KeyBody> {
        if self.r#type != ObjectType::Key {
            return Err(Error::invalid_data("object is not a Key"));
        }
        KeyBody::decode(&self.body)
    }

    pub fn get_macro_body(&self) -> Result<MacroBody> {
        if self.r#type != ObjectType::Macro {
            return Err(Error::invalid_data("object is not a Macro"));
        }
        MacroBody::decode(&self.body)
    }

    pub fn get_alarm_mask_body(&self) -> Result<AlarmMaskBody> {
        if self.r#type != ObjectType::AlarmMask {
            return Err(Error::invalid_data("object is not an Alarm Mask"));
        }
        AlarmMaskBody::decode(&self.body)
    }

    pub fn get_data_mask_body(&self) -> Result<DataMaskBody> {
        if self.r#type != ObjectType::DataMask {
            return Err(Error::invalid_data("object is not a Data Mask"));
        }
        DataMaskBody::decode(&self.body)
    }

    pub fn get_container_body(&self) -> Result<ContainerBody> {
        if self.r#type != ObjectType::Container {
            return Err(Error::invalid_data("object is not a Container"));
        }
        ContainerBody::decode(&self.body)
    }

    pub fn get_soft_key_mask_body(&self) -> Result<SoftKeyMaskBody> {
        if self.r#type != ObjectType::SoftKeyMask {
            return Err(Error::invalid_data("object is not a Soft Key Mask"));
        }
        SoftKeyMaskBody::decode(&self.body)
    }

    pub fn get_button_body(&self) -> Result<ButtonBody> {
        if self.r#type != ObjectType::Button {
            return Err(Error::invalid_data("object is not a Button"));
        }
        ButtonBody::decode(&self.body)
    }

    pub fn get_number_variable_body(&self) -> Result<NumberVariableBody> {
        if self.r#type != ObjectType::NumberVariable {
            return Err(Error::invalid_data("object is not a Number Variable"));
        }
        NumberVariableBody::decode(&self.body)
    }

    pub fn get_string_variable_body(&self) -> Result<StringVariableBody> {
        if self.r#type != ObjectType::StringVariable {
            return Err(Error::invalid_data("object is not a String Variable"));
        }
        Ok(StringVariableBody::decode(&self.body))
    }

    pub fn get_font_attributes_body(&self) -> Result<FontAttributesBody> {
        if self.r#type != ObjectType::FontAttributes {
            return Err(Error::invalid_data("object is not Font Attributes"));
        }
        FontAttributesBody::decode(&self.body)
    }

    pub fn get_line_attributes_body(&self) -> Result<LineAttributesBody> {
        if self.r#type != ObjectType::LineAttributes {
            return Err(Error::invalid_data("object is not Line Attributes"));
        }
        LineAttributesBody::decode(&self.body)
    }

    pub fn get_fill_attributes_body(&self) -> Result<FillAttributesBody> {
        if self.r#type != ObjectType::FillAttributes {
            return Err(Error::invalid_data("object is not Fill Attributes"));
        }
        FillAttributesBody::decode(&self.body)
    }

    pub fn get_input_attributes_body(&self) -> Result<InputAttributesBody> {
        if self.r#type != ObjectType::InputAttributes {
            return Err(Error::invalid_data("object is not Input Attributes"));
        }
        InputAttributesBody::decode(&self.body)
    }

    pub fn get_extended_input_attributes_body(&self) -> Result<ExtendedInputAttributesBody> {
        if self.r#type != ObjectType::ExtendedInputAttributes {
            return Err(Error::invalid_data(
                "object is not Extended Input Attributes",
            ));
        }
        ExtendedInputAttributesBody::decode(&self.body)
    }

    pub fn get_object_pointer_body(&self) -> Result<ObjectPointerBody> {
        if self.r#type != ObjectType::ObjectPointer {
            return Err(Error::invalid_data("object is not an Object Pointer"));
        }
        ObjectPointerBody::decode(&self.body)
    }

    pub fn get_output_line_body(&self) -> Result<OutputLineBody> {
        if self.r#type != ObjectType::Line {
            return Err(Error::invalid_data("object is not an Output Line"));
        }
        OutputLineBody::decode(&self.body)
    }

    pub fn get_output_rectangle_body(&self) -> Result<OutputRectangleBody> {
        if self.r#type != ObjectType::Rectangle {
            return Err(Error::invalid_data("object is not an Output Rectangle"));
        }
        OutputRectangleBody::decode(&self.body)
    }

    pub fn get_output_ellipse_body(&self) -> Result<OutputEllipseBody> {
        if self.r#type != ObjectType::Ellipse {
            return Err(Error::invalid_data("object is not an Output Ellipse"));
        }
        OutputEllipseBody::decode(&self.body)
    }

    pub fn get_output_polygon_body(&self) -> Result<OutputPolygonBody> {
        if self.r#type != ObjectType::Polygon {
            return Err(Error::invalid_data("object is not an Output Polygon"));
        }
        OutputPolygonBody::decode(&self.body)
    }

    pub fn get_input_boolean_body(&self) -> Result<InputBooleanBody> {
        if self.r#type != ObjectType::InputBoolean {
            return Err(Error::invalid_data("object is not an Input Boolean"));
        }
        InputBooleanBody::decode(&self.body)
    }

    pub fn get_input_string_body(&self) -> Result<InputStringBody> {
        if self.r#type != ObjectType::InputString {
            return Err(Error::invalid_data("object is not an Input String"));
        }
        InputStringBody::decode(&self.body)
    }

    pub fn get_input_number_body(&self) -> Result<InputNumberBody> {
        if self.r#type != ObjectType::InputNumber {
            return Err(Error::invalid_data("object is not an Input Number"));
        }
        InputNumberBody::decode(&self.body)
    }

    pub fn get_input_list_body(&self) -> Result<InputListBody> {
        if self.r#type != ObjectType::InputList {
            return Err(Error::invalid_data("object is not an Input List"));
        }
        InputListBody::decode(&self.body)
    }

    pub fn get_output_list_body(&self) -> Result<OutputListBody> {
        if self.r#type != ObjectType::OutputList {
            return Err(Error::invalid_data("object is not an Output List"));
        }
        OutputListBody::decode(&self.body)
    }

    pub fn get_output_string_body(&self) -> Result<OutputStringBody> {
        if self.r#type != ObjectType::OutputString {
            return Err(Error::invalid_data("object is not an Output String"));
        }
        OutputStringBody::decode(&self.body)
    }

    pub fn get_output_number_body(&self) -> Result<OutputNumberBody> {
        if self.r#type != ObjectType::OutputNumber {
            return Err(Error::invalid_data("object is not an Output Number"));
        }
        OutputNumberBody::decode(&self.body)
    }

    pub fn get_meter_body(&self) -> Result<MeterBody> {
        if self.r#type != ObjectType::Meter {
            return Err(Error::invalid_data("object is not a Meter"));
        }
        MeterBody::decode(&self.body)
    }

    pub fn get_linear_bar_graph_body(&self) -> Result<LinearBarGraphBody> {
        if self.r#type != ObjectType::LinearBarGraph {
            return Err(Error::invalid_data("object is not a Linear Bar Graph"));
        }
        LinearBarGraphBody::decode(&self.body)
    }

    pub fn get_arched_bar_graph_body(&self) -> Result<ArchedBarGraphBody> {
        if self.r#type != ObjectType::ArchedBarGraph {
            return Err(Error::invalid_data("object is not an Arched Bar Graph"));
        }
        ArchedBarGraphBody::decode(&self.body)
    }

    pub fn get_picture_graphic_body(&self) -> Result<PictureGraphicBody> {
        if self.r#type != ObjectType::PictureGraphic {
            return Err(Error::invalid_data("object is not a Picture Graphic"));
        }
        PictureGraphicBody::decode(&self.body)
    }

    pub fn get_working_set_body(&self) -> Result<WorkingSetBody> {
        if self.r#type != ObjectType::WorkingSet {
            return Err(Error::invalid_data("object is not a Working Set"));
        }
        WorkingSetBody::decode(&self.body)
    }

    pub fn get_aux_function_body(&self) -> Result<AuxFunctionBody> {
        if self.r#type != ObjectType::AuxFunction {
            return Err(Error::invalid_data("object is not an Aux Function"));
        }
        AuxFunctionBody::decode(&self.body)
    }

    pub fn get_aux_input_body(&self) -> Result<AuxInputBody> {
        if self.r#type != ObjectType::AuxInput {
            return Err(Error::invalid_data("object is not an Aux Input"));
        }
        AuxInputBody::decode(&self.body)
    }

    pub fn get_aux_function2_body(&self) -> Result<AuxFunction2Body> {
        if self.r#type != ObjectType::AuxFunction2 {
            return Err(Error::invalid_data("object is not an Aux Function 2"));
        }
        AuxFunction2Body::decode(&self.body)
    }

    pub fn get_aux_input2_body(&self) -> Result<AuxInput2Body> {
        if self.r#type != ObjectType::AuxInput2 {
            return Err(Error::invalid_data("object is not an Aux Input 2"));
        }
        AuxInput2Body::decode(&self.body)
    }

    pub fn get_aux_control_designator_body(&self) -> Result<AuxControlDesignatorBody> {
        if self.r#type != ObjectType::AuxControlDesig {
            return Err(Error::invalid_data(
                "object is not an Aux Control Designator",
            ));
        }
        AuxControlDesignatorBody::decode(&self.body)
    }

    pub fn get_graphic_data_body(&self) -> Result<GraphicDataBody> {
        if self.r#type != ObjectType::GraphicData {
            return Err(Error::invalid_data("object is not Graphic Data"));
        }
        GraphicDataBody::decode(&self.body)
    }

    pub fn get_scaled_graphic_body(&self) -> Result<ScaledGraphicBody> {
        if self.r#type != ObjectType::ScaledGraphic {
            return Err(Error::invalid_data("object is not a Scaled Graphic"));
        }
        ScaledGraphicBody::decode(&self.body)
    }

    pub fn get_scaled_bitmap_body(&self) -> Result<ScaledBitmapBody> {
        if self.r#type != ObjectType::ScaledBitmap {
            return Err(Error::invalid_data("object is not a Scaled Bitmap"));
        }
        ScaledBitmapBody::decode(&self.body)
    }

    pub fn get_animation_body(&self) -> Result<AnimationBody> {
        if self.r#type != ObjectType::Animation {
            return Err(Error::invalid_data("object is not an Animation"));
        }
        AnimationBody::decode(&self.body)
    }

    pub fn get_colour_map_body(&self) -> Result<ColourMapBody> {
        if self.r#type != ObjectType::ColourMap {
            return Err(Error::invalid_data("object is not a Colour Map"));
        }
        ColourMapBody::decode(&self.body)
    }

    pub fn get_graphic_context_body(&self) -> Result<GraphicContextBody> {
        if self.r#type != ObjectType::GraphicContext {
            return Err(Error::invalid_data("object is not a Graphic Context"));
        }
        GraphicContextBody::decode(&self.body)
    }

    pub fn get_external_object_definition_body(&self) -> Result<ExternalObjectDefinitionBody> {
        if self.r#type != ObjectType::ExternalObjectDefinition {
            return Err(Error::invalid_data(
                "object is not an External Object Definition",
            ));
        }
        ExternalObjectDefinitionBody::decode(&self.body)
    }

    pub fn get_external_reference_name_body(&self) -> Result<ExternalReferenceNameBody> {
        if self.r#type != ObjectType::ExternalReferenceName {
            return Err(Error::invalid_data(
                "object is not an External Reference Name",
            ));
        }
        ExternalReferenceNameBody::decode(&self.body)
    }

    pub fn get_external_object_pointer_body(&self) -> Result<ExternalObjectPointerBody> {
        if self.r#type != ObjectType::ExternalObjectPointer {
            return Err(Error::invalid_data(
                "object is not an External Object Pointer",
            ));
        }
        ExternalObjectPointerBody::decode(&self.body)
    }

    pub fn get_colour_palette_body(&self) -> Result<ColourPaletteBody> {
        if self.r#type != ObjectType::ColourPalette {
            return Err(Error::invalid_data("object is not a Colour Palette"));
        }
        ColourPaletteBody::decode(&self.body)
    }

    pub fn get_working_set_special_controls_body(&self) -> Result<WorkingSetSpecialControlsBody> {
        if self.r#type != ObjectType::WorkingSetSpecialControls {
            return Err(Error::invalid_data(
                "object is not Working Set Special Controls",
            ));
        }
        WorkingSetSpecialControlsBody::decode(&self.body)
    }

    pub fn get_graphics_context_body(&self) -> Result<GraphicsContextBody> {
        if self.r#type != ObjectType::GraphicsContext {
            return Err(Error::invalid_data("object is not a Graphics Context"));
        }
        GraphicsContextBody::decode(&self.body)
    }

    pub fn get_object_label_ref_body(&self) -> Result<ObjectLabelRefBody> {
        if self.r#type != ObjectType::ObjectLabelRef {
            return Err(Error::invalid_data("object is not an Object Label Ref"));
        }
        ObjectLabelRefBody::decode(&self.body)
    }

    /// Validate that the byte body is decodable for this object's declared
    /// type. This is intentionally separate from graph validation so callers
    /// can reject malformed object pools before activating them.
    pub fn validate_body(&self) -> Result<()> {
        match self.r#type {
            ObjectType::WorkingSet => self.get_working_set_body().map(|_| ()),
            ObjectType::DataMask => self.get_data_mask_body().map(|_| ()),
            ObjectType::AlarmMask => self.get_alarm_mask_body().map(|_| ()),
            ObjectType::Container => self.get_container_body().map(|_| ()),
            ObjectType::SoftKeyMask => self.get_soft_key_mask_body().map(|_| ()),
            ObjectType::Key => self.get_key_body().map(|_| ()),
            ObjectType::Button => self.get_button_body().map(|_| ()),
            ObjectType::InputBoolean => self.get_input_boolean_body().map(|_| ()),
            ObjectType::InputString => self.get_input_string_body().map(|_| ()),
            ObjectType::InputNumber => self.get_input_number_body().map(|_| ()),
            ObjectType::InputList => self.get_input_list_body().map(|_| ()),
            ObjectType::OutputList => self.get_output_list_body().map(|_| ()),
            ObjectType::OutputString => self.get_output_string_body().map(|_| ()),
            ObjectType::OutputNumber => self.get_output_number_body().map(|_| ()),
            ObjectType::Line => self.get_output_line_body().map(|_| ()),
            ObjectType::Rectangle => self.get_output_rectangle_body().map(|_| ()),
            ObjectType::Ellipse => self.get_output_ellipse_body().map(|_| ()),
            ObjectType::Polygon => self.get_output_polygon_body().map(|_| ()),
            ObjectType::Meter => self.get_meter_body().map(|_| ()),
            ObjectType::LinearBarGraph => self.get_linear_bar_graph_body().map(|_| ()),
            ObjectType::ArchedBarGraph => self.get_arched_bar_graph_body().map(|_| ()),
            ObjectType::PictureGraphic => self.get_picture_graphic_body().map(|_| ()),
            ObjectType::NumberVariable => self.get_number_variable_body().map(|_| ()),
            ObjectType::StringVariable => self.get_string_variable_body().map(|_| ()),
            ObjectType::FontAttributes => self.get_font_attributes_body().map(|_| ()),
            ObjectType::LineAttributes => self.get_line_attributes_body().map(|_| ()),
            ObjectType::FillAttributes => self.get_fill_attributes_body().map(|_| ()),
            ObjectType::InputAttributes => self.get_input_attributes_body().map(|_| ()),
            ObjectType::ExtendedInputAttributes => {
                self.get_extended_input_attributes_body().map(|_| ())
            }
            ObjectType::ObjectPointer => self.get_object_pointer_body().map(|_| ()),
            ObjectType::Macro => self.get_macro_body().map(|_| ()),
            ObjectType::AuxFunction => self.get_aux_function_body().map(|_| ()),
            ObjectType::AuxInput => self.get_aux_input_body().map(|_| ()),
            ObjectType::AuxFunction2 => self.get_aux_function2_body().map(|_| ()),
            ObjectType::AuxInput2 => self.get_aux_input2_body().map(|_| ()),
            ObjectType::AuxControlDesig => self.get_aux_control_designator_body().map(|_| ()),
            ObjectType::WindowMask => self.get_window_mask_body().map(|_| ()),
            ObjectType::KeyGroup => self.get_key_group_body().map(|_| ()),
            ObjectType::GraphicData => self.get_graphic_data_body().map(|_| ()),
            ObjectType::ScaledGraphic => self.get_scaled_graphic_body().map(|_| ()),
            ObjectType::Animation => self.get_animation_body().map(|_| ()),
            ObjectType::ColourMap => self.get_colour_map_body().map(|_| ()),
            ObjectType::GraphicContext => self.get_graphic_context_body().map(|_| ()),
            ObjectType::ExternalObjectDefinition => {
                ExternalObjectDefinitionBody::decode(&self.body).map(|_| ())
            }
            ObjectType::ExternalReferenceName => {
                self.get_external_reference_name_body().map(|_| ())
            }
            ObjectType::ExternalObjectPointer => {
                self.get_external_object_pointer_body().map(|_| ())
            }
            ObjectType::ColourPalette => self.get_colour_palette_body().map(|_| ()),
            ObjectType::WorkingSetSpecialControls => {
                self.get_working_set_special_controls_body().map(|_| ())
            }
            ObjectType::GraphicsContext => self.get_graphics_context_body().map(|_| ()),
            ObjectType::ObjectLabelRef => self.get_object_label_ref_body().map(|_| ()),
            ObjectType::ScaledBitmap => self.get_scaled_bitmap_body().map(|_| ()),
        }
    }

    /// Serialize this object to its on-wire byte sequence.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let is_parent = object_type_may_have_children(self.r#type);
        if self.r#type == ObjectType::WorkingSet {
            return self.serialize_working_set();
        }
        if self.r#type == ObjectType::PictureGraphic {
            return Ok(self.serialize_picture_graphic());
        }
        let record_size = parent_record_size(self.r#type).unwrap_or(0);
        let children_size = if is_parent {
            serialized_child_macro_tail_len(
                self.children_pos.len(),
                self.macros.len(),
                record_size,
            )?
        } else {
            0
        };
        let body_len = self
            .body
            .len()
            .checked_add(children_size)
            .ok_or_else(|| Error::invalid_data("VT object serialized body length overflows"))?;

        // ISO 11783-6 object pool layout: `[id:u16][type:u8][body…]` with no
        // per-object length prefix. Object boundaries are recovered by the
        // parse-by-type walker (`object_body_total_len`).
        let capacity = 3usize
            .checked_add(body_len)
            .ok_or_else(|| Error::invalid_data("VT object serialized length overflows"))?;
        let mut data = Vec::with_capacity(capacity);
        push_u16_le(&mut data, self.id);
        data.push(self.r#type.as_u8());
        data.extend_from_slice(&self.body);
        if is_parent {
            // ISO 11783-6 parent tail — both counts precede both lists:
            //   `[num_objects:u8][num_macros:u8][object records][macro records]`.
            data.push(self.children_pos.len() as u8);
            data.push(self.macros.len() as u8);
            for cref in &self.children_pos {
                push_u16_le(&mut data, cref.id);
                if record_size == 6 {
                    data.extend_from_slice(&cref.x.to_le_bytes());
                    data.extend_from_slice(&cref.y.to_le_bytes());
                }
            }
            for mref in &self.macros {
                data.push(mref.event_id);
                data.push(mref.macro_id);
            }
        } else if leaf_has_macro_tail(self.r#type) {
            // Leaf objects carry only the trailing `[num_macros][macro refs]`.
            data.push(self.macros.len() as u8);
            for mref in &self.macros {
                data.push(mref.event_id);
                data.push(mref.macro_id);
            }
        }
        Ok(data)
    }

    /// Picture Graphic has an interleaved tail — `[13 header incl data-length]
    /// [num_macros][raw data][macro refs]`. `self.body` stores `[13 header][data]`,
    /// so the macro count is re-inserted before the pixel data on the wire.
    fn serialize_picture_graphic(&self) -> Vec<u8> {
        let split = self.body.len().min(13);
        let mut data = Vec::with_capacity(3 + self.body.len() + 1 + self.macros.len() * 2);
        push_u16_le(&mut data, self.id);
        data.push(self.r#type.as_u8());
        data.extend_from_slice(&self.body[..split]);
        data.push(self.macros.len() as u8);
        data.extend_from_slice(&self.body[split..]);
        for mref in &self.macros {
            data.push(mref.event_id);
            data.push(mref.macro_id);
        }
        data
    }

    fn serialize_working_set(&self) -> Result<Vec<u8>> {
        let body = self.get_working_set_body()?;
        if self.children_pos.is_empty() || self.children_pos.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Working Set object count must fit the 1..=255 count field",
            ));
        }
        if self.macros.len() > u8::MAX as usize || body.languages.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Working Set macro/language count exceeds u8 count field",
            ));
        }
        let child_bytes = self
            .children_pos
            .len()
            .checked_mul(6)
            .ok_or_else(|| Error::invalid_data("Working Set child-list length overflows"))?;
        let macro_bytes = self
            .macros
            .len()
            .checked_mul(2)
            .ok_or_else(|| Error::invalid_data("Working Set macro-list length overflows"))?;
        let language_bytes = body
            .languages
            .len()
            .checked_mul(2)
            .ok_or_else(|| Error::invalid_data("Working Set language-list length overflows"))?;
        let body_len = 4usize
            .checked_add(3)
            .and_then(|n| n.checked_add(child_bytes))
            .and_then(|n| n.checked_add(macro_bytes))
            .and_then(|n| n.checked_add(language_bytes))
            .ok_or_else(|| Error::invalid_data("Working Set serialized body length overflows"))?;
        let mut data = Vec::with_capacity(3 + body_len);
        push_u16_le(&mut data, self.id);
        data.push(self.r#type.as_u8());
        data.push(body.background_colour);
        data.push(body.selectable);
        push_u16_le(&mut data, body.active_mask);
        data.push(self.children_pos.len() as u8);
        data.push(self.macros.len() as u8);
        data.push(body.languages.len() as u8);
        for cref in &self.children_pos {
            push_u16_le(&mut data, cref.id);
            data.extend_from_slice(&cref.x.to_le_bytes());
            data.extend_from_slice(&cref.y.to_le_bytes());
        }
        for mref in &self.macros {
            data.push(mref.event_id);
            data.push(mref.macro_id);
        }
        for language in &body.languages {
            data.extend_from_slice(language);
        }
        Ok(data)
    }
}

// ─── ObjectPool ───────────────────────────────────────────────────────

/// A collection of VT objects with a version label.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectPool {
    objects: Vec<VTObject>,
    version_label: String,
}

