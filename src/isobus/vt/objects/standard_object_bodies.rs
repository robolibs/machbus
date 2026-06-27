/// Output Ellipse body (Type 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputEllipseBody {
    pub width: u16,
    pub height: u16,
    pub line_attributes: ObjectID,
    /// 0 = closed, 1 = open by start/end, 2 = segment, 3 = section.
    pub ellipse_type: u8,
    /// Half-degrees from positive X axis, 0..=180.
    pub start_angle: u8,
    /// Half-degrees from positive X axis, 0..=180.
    pub end_angle: u8,
    pub fill_attributes: ObjectID,
}

impl Default for OutputEllipseBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            line_attributes: ObjectID::NULL,
            ellipse_type: 0,
            start_angle: 0,
            end_angle: 0,
            fill_attributes: ObjectID::NULL,
        }
    }
}

impl OutputEllipseBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.ellipse_type > 3 {
            return Err(Error::invalid_data("Output Ellipse type must be in 0..=3"));
        }
        if self.start_angle > 180 || self.end_angle > 180 {
            return Err(Error::invalid_data(
                "Output Ellipse angles must be half-degree values in 0..=180",
            ));
        }
        // ISO 11783-6 order: line attributes, width, height, type, angles, fill.
        let mut data = Vec::with_capacity(11);
        push_u16_le(&mut data, self.line_attributes);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.ellipse_type);
        data.push(self.start_angle);
        data.push(self.end_angle);
        push_u16_le(&mut data, self.fill_attributes);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 11 {
            return Err(Error::invalid_data("Output Ellipse body too short"));
        }
        if body[6] > 3 {
            return Err(Error::invalid_data(
                "Output Ellipse body contains reserved ellipse type",
            ));
        }
        if body[7] > 180 || body[8] > 180 {
            return Err(Error::invalid_data(
                "Output Ellipse body contains out-of-range half-degree angle",
            ));
        }
        Ok(Self {
            line_attributes: u16_le_as(&body[0..]),
            width: u16_le(&body[2..]),
            height: u16_le(&body[4..]),
            ellipse_type: body[6],
            start_angle: body[7],
            end_angle: body[8],
            fill_attributes: u16_le_as(&body[9..]),
        })
    }
}

/// One point in an [`OutputPolygonBody`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PolygonPoint {
    pub x: u16,
    pub y: u16,
}

/// Output Polygon body (Type 16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputPolygonBody {
    pub width: u16,
    pub height: u16,
    pub line_attributes: ObjectID,
    pub fill_attributes: ObjectID,
    /// 0 = convex, 1 = non-convex, 2 = complex, 3 = open.
    pub polygon_type: u8,
    pub points: Vec<PolygonPoint>,
}

impl Default for OutputPolygonBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            line_attributes: ObjectID::NULL,
            fill_attributes: ObjectID::NULL,
            polygon_type: 0,
            points: Vec::new(),
        }
    }
}

impl OutputPolygonBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.polygon_type > 3 {
            return Err(Error::invalid_data("Output Polygon type must be in 0..=3"));
        }
        if self.points.len() < 3 {
            return Err(Error::invalid_data(
                "Output Polygon must contain at least three points",
            ));
        }
        if self.points.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Output Polygon point count exceeds u8 count field",
            ));
        }
        let mut data = Vec::with_capacity(10 + self.points.len() * 4);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        push_u16_le(&mut data, self.line_attributes);
        push_u16_le(&mut data, self.fill_attributes);
        data.push(self.polygon_type);
        data.push(self.points.len() as u8);
        for point in &self.points {
            push_u16_le(&mut data, point.x);
            push_u16_le(&mut data, point.y);
        }
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 10 {
            return Err(Error::invalid_data("Output Polygon body too short"));
        }
        if body[8] > 3 {
            return Err(Error::invalid_data(
                "Output Polygon body contains reserved polygon type",
            ));
        }
        let point_count = body[9] as usize;
        if point_count < 3 {
            return Err(Error::invalid_data(
                "Output Polygon body has fewer than three points",
            ));
        }
        let point_bytes = point_count
            .checked_mul(4)
            .ok_or_else(|| Error::invalid_data("Output Polygon point-list length overflows"))?;
        if body.len() != 10 + point_bytes {
            return Err(Error::invalid_data(
                "Output Polygon body point count does not match body length",
            ));
        }
        let mut points = Vec::with_capacity(point_count);
        let mut offset = 10;
        for _ in 0..point_count {
            points.push(PolygonPoint {
                x: u16_le(&body[offset..]),
                y: u16_le(&body[offset + 2..]),
            });
            offset += 4;
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            line_attributes: u16_le_as(&body[4..]),
            fill_attributes: u16_le_as(&body[6..]),
            polygon_type: body[8],
            points,
        })
    }
}

/// Input Boolean body (Type 7). ISO 11783-6 fixed fields: background
/// colour, width, a Font Attributes object id (foreground), a Number
/// Variable reference, the current value, and the enabled flag. There
/// is no separate height field (the input is square in `width`), and
/// the foreground is a Font Attributes reference (not a colour byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputBooleanBody {
    pub background_color: u8,
    pub width: u16,
    /// Object ID of a Font Attributes object (foreground rendering).
    pub foreground: ObjectID,
    pub variable_reference: ObjectID,
    /// 0 = FALSE, 1 = TRUE.
    pub value: u8,
    /// 0 = disabled, 1 = enabled.
    pub enabled: u8,
}

impl Default for InputBooleanBody {
    fn default() -> Self {
        Self {
            background_color: 0,
            width: 0,
            foreground: ObjectID::NULL,
            variable_reference: ObjectID::NULL,
            value: 0,
            enabled: 0,
        }
    }
}

impl InputBooleanBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.value > 1 {
            return Err(Error::invalid_data("Input Boolean value must be 0 or 1"));
        }
        if self.enabled > 1 {
            return Err(Error::invalid_data("Input Boolean enabled must be 0 or 1"));
        }
        let mut data = Vec::with_capacity(9);
        data.push(self.background_color);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.foreground);
        push_u16_le(&mut data, self.variable_reference);
        data.push(self.value);
        data.push(self.enabled);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 9 {
            return Err(Error::invalid_data("Input Boolean body too short"));
        }
        if body[7] > 1 {
            return Err(Error::invalid_data(
                "Input Boolean body contains invalid value",
            ));
        }
        if body[8] > 1 {
            return Err(Error::invalid_data(
                "Input Boolean body contains invalid enabled value",
            ));
        }
        Ok(Self {
            background_color: body[0],
            width: u16_le(&body[1..]),
            foreground: u16_le_as(&body[3..]),
            variable_reference: u16_le_as(&body[5..]),
            value: body[7],
            enabled: body[8],
        })
    }
}

/// Input String body (Type 8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputStringBody {
    pub width: u16,
    pub height: u16,
    pub background_color: u8,
    pub font_attributes: ObjectID,
    pub input_attributes: ObjectID,
    /// ISO 11783-6 option bitmask. VT v4 and later permit bit 2
    /// ("wrap on hyphen") in addition to the older low two bits.
    pub options: u8,
    pub variable_reference: ObjectID,
    /// Bits 0-1 = horizontal justification, bits 2-3 = vertical justification.
    pub justification: u8,
    /// Maximum fixed length of the input string in bytes (ISO 11783-6 u8).
    pub max_length: u8,
}

impl Default for InputStringBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            background_color: 0,
            font_attributes: ObjectID::NULL,
            input_attributes: ObjectID::NULL,
            options: 0,
            variable_reference: ObjectID::NULL,
            justification: 0,
            max_length: 0,
        }
    }
}

impl InputStringBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x07 != 0 {
            return Err(Error::invalid_data(
                "Input String options contain reserved bits",
            ));
        }
        if !text_justification_is_valid(self.justification) {
            return Err(Error::invalid_data(
                "Input String justification contains reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(14);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.background_color);
        push_u16_le(&mut data, self.font_attributes);
        push_u16_le(&mut data, self.input_attributes);
        data.push(self.options);
        push_u16_le(&mut data, self.variable_reference);
        data.push(self.justification);
        data.push(self.max_length);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 14 {
            return Err(Error::invalid_data("Input String body too short"));
        }
        if body[9] & !0x07 != 0 {
            return Err(Error::invalid_data(
                "Input String body contains reserved option bits",
            ));
        }
        if !text_justification_is_valid(body[12]) {
            return Err(Error::invalid_data(
                "Input String body contains reserved justification",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            background_color: body[4],
            font_attributes: u16_le_as(&body[5..]),
            input_attributes: u16_le_as(&body[7..]),
            options: body[9],
            variable_reference: u16_le_as(&body[10..]),
            justification: body[12],
            max_length: body[13],
        })
    }
}

/// Input Number body (Type 9). ISO 11783-6 layout: width, height,
/// background, font attributes, options 1, variable reference, raw value,
/// min, max, offset, scale (f32), decimals, format, justification.
/// and options 2.
/// (An earlier revision carried a non-standard `input_attributes` field
/// here; it has been removed for conformance — Input Number has no
/// input-attributes reference.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputNumberBody {
    pub width: u16,
    pub height: u16,
    pub background_color: u8,
    pub font_attributes: ObjectID,
    /// Options 1: bit 0 = transparent, bit 1 = leading zeros,
    /// bit 2 = zero-as-blank, bit 3 = truncate.
    pub options: u8,
    pub variable_reference: ObjectID,
    /// Raw value used only when `variable_reference` is NULL.
    pub value: u32,
    pub min_value: i32,
    pub max_value: i32,
    pub offset: i32,
    /// Scaling factor applied as `displayed = (value + offset) * scale`.
    /// ISO 11783-6 defines this field as an IEEE-754 single-precision
    /// float; `1.0` means identity scaling.
    pub scale: f32,
    pub number_of_decimals: u8,
    /// 0 = fixed decimal, 1 = exponential.
    pub format: u8,
    /// Bits 0-1 = horizontal justification, bits 2-3 = vertical justification.
    pub justification: u8,
    /// Options 2: bit 0 = enabled, bit 1 = real-time editing.
    pub options2: u8,
}

impl Default for InputNumberBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            background_color: 0,
            font_attributes: ObjectID::NULL,
            options: 0,
            variable_reference: ObjectID::NULL,
            value: 0,
            min_value: 0,
            max_value: 0,
            offset: 0,
            scale: 1.0,
            number_of_decimals: 0,
            format: 0,
            justification: 0,
            options2: 0,
        }
    }
}

impl InputNumberBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x0F != 0 {
            return Err(Error::invalid_data(
                "Input Number options contain reserved bits",
            ));
        }
        if self.options2 & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Input Number options2 contain reserved bits",
            ));
        }
        if self.min_value > self.max_value {
            return Err(Error::invalid_data(
                "Input Number min value must be <= max value",
            ));
        }
        if !self.scale.is_finite() {
            return Err(Error::invalid_data("Input Number scale must be finite"));
        }
        if self.number_of_decimals > 7 {
            return Err(Error::invalid_data(
                "Input Number number of decimals must be in 0..=7",
            ));
        }
        if self.format > 1 {
            return Err(Error::invalid_data("Input Number format must be 0 or 1"));
        }
        if !text_justification_is_valid(self.justification) {
            return Err(Error::invalid_data(
                "Input Number justification contains reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(34);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.background_color);
        push_u16_le(&mut data, self.font_attributes);
        data.push(self.options);
        push_u16_le(&mut data, self.variable_reference);
        data.extend_from_slice(&self.value.to_le_bytes());
        push_i32_le(&mut data, self.min_value);
        push_i32_le(&mut data, self.max_value);
        push_i32_le(&mut data, self.offset);
        data.extend_from_slice(&self.scale.to_le_bytes());
        data.push(self.number_of_decimals);
        data.push(self.format);
        data.push(self.justification);
        data.push(self.options2);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 34 {
            return Err(Error::invalid_data("Input Number body too short"));
        }
        if body[7] & !0x0F != 0 {
            return Err(Error::invalid_data(
                "Input Number body contains reserved option bits",
            ));
        }
        if body[33] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Input Number body contains reserved options2 bits",
            ));
        }
        let min_value = i32_le(&body[14..]);
        let max_value = i32_le(&body[18..]);
        if min_value > max_value {
            return Err(Error::invalid_data(
                "Input Number body has min value greater than max value",
            ));
        }
        let scale = f32::from_le_bytes([body[26], body[27], body[28], body[29]]);
        if !scale.is_finite() {
            return Err(Error::invalid_data(
                "Input Number body has non-finite scale",
            ));
        }
        if body[30] > 7 {
            return Err(Error::invalid_data(
                "Input Number body contains reserved number of decimals",
            ));
        }
        if body[31] > 1 {
            return Err(Error::invalid_data(
                "Input Number body contains reserved format",
            ));
        }
        if !text_justification_is_valid(body[32]) {
            return Err(Error::invalid_data(
                "Input Number body contains reserved justification",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            background_color: body[4],
            font_attributes: u16_le_as(&body[5..]),
            options: body[7],
            variable_reference: u16_le_as(&body[8..]),
            value: u32::from_le_bytes([body[10], body[11], body[12], body[13]]),
            min_value,
            max_value,
            offset: i32_le(&body[22..]),
            scale,
            number_of_decimals: body[30],
            format: body[31],
            justification: body[32],
            options2: body[33],
        })
    }
}

/// Input List body (Type 10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputListBody {
    pub width: u16,
    pub height: u16,
    pub variable_reference: ObjectID,
    /// Selected list index used only when `variable_reference` is NULL.
    /// `255` means no item is chosen.
    pub value: u8,
    /// Bit 0 = enabled, bit 1 = real-time editing.
    pub options: u8,
    pub items: Vec<ObjectID>,
}

impl Default for InputListBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            variable_reference: ObjectID::NULL,
            value: 255,
            options: 0,
            items: Vec::new(),
        }
    }
}

impl InputListBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Input List options contain reserved bits",
            ));
        }
        if self.items.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Input List item count exceeds u8 count field",
            ));
        }
        let mut data = Vec::with_capacity(9 + self.items.len() * 2);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        push_u16_le(&mut data, self.variable_reference);
        data.push(self.value);
        data.push(self.items.len() as u8);
        data.push(self.options);
        for item in &self.items {
            push_u16_le(&mut data, *item);
        }
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 9 {
            return Err(Error::invalid_data("Input List body too short"));
        }
        if body[8] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Input List body contains reserved option bits",
            ));
        }
        let count = usize::from(body[7]);
        let item_bytes = count
            .checked_mul(2)
            .ok_or_else(|| Error::invalid_data("Input List item-list length overflows"))?;
        if body.len() != 9 + item_bytes {
            return Err(Error::invalid_data(
                "Input List item count does not match body length",
            ));
        }
        let mut items = Vec::with_capacity(count);
        let mut offset = 9;
        for _ in 0..count {
            items.push(u16_le_as(&body[offset..]));
            offset += 2;
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            variable_reference: u16_le_as(&body[4..]),
            value: body[6],
            options: body[8],
            items,
        })
    }
}

/// Output List body (Type 37, VT version 4+).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputListBody {
    pub width: u16,
    pub height: u16,
    pub variable_reference: ObjectID,
    /// Selected list index used only when `variable_reference` is NULL.
    /// `255` means no item is chosen.
    pub value: u8,
    pub items: Vec<ObjectID>,
}

impl Default for OutputListBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            variable_reference: ObjectID::NULL,
            value: 255,
            items: Vec::new(),
        }
    }
}

impl OutputListBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.items.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Output List item count exceeds u8 count field",
            ));
        }
        let mut data = Vec::with_capacity(8 + self.items.len() * 2);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        push_u16_le(&mut data, self.variable_reference);
        data.push(self.value);
        data.push(self.items.len() as u8);
        for item in &self.items {
            push_u16_le(&mut data, *item);
        }
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 8 {
            return Err(Error::invalid_data("Output List body too short"));
        }
        let count = body[7] as usize;
        let item_bytes = count
            .checked_mul(2)
            .ok_or_else(|| Error::invalid_data("Output List item-list length overflows"))?;
        if body.len() != 8 + item_bytes {
            return Err(Error::invalid_data(
                "Output List item count does not match body length",
            ));
        }
        let mut items = Vec::with_capacity(count);
        let mut offset = 8;
        for _ in 0..count {
            items.push(u16_le_as(&body[offset..]));
            offset += 2;
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            variable_reference: u16_le_as(&body[4..]),
            value: body[6],
            items,
        })
    }
}

/// Output String body (Type 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputStringBody {
    pub width: u16,
    pub height: u16,
    pub background_color: u8,
    pub font_attributes: ObjectID,
    /// Bit 0 = transparent background, bit 1 = horizontal scroll.
    pub options: u8,
    pub variable_reference: ObjectID,
    /// Bits 0-1 = horizontal justification, bits 2-3 = vertical justification.
    pub justification: u8,
    pub value: Vec<u8>,
}

impl Default for OutputStringBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            background_color: 0,
            font_attributes: ObjectID::NULL,
            options: 0,
            variable_reference: ObjectID::NULL,
            justification: 0,
            value: Vec::new(),
        }
    }
}

impl OutputStringBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Output String options contain reserved bits",
            ));
        }
        if !text_justification_is_valid(self.justification) {
            return Err(Error::invalid_data(
                "Output String justification contains reserved bits",
            ));
        }
        if self.value.len() > u16::MAX as usize {
            return Err(Error::invalid_data(
                "Output String value exceeds u16 length field",
            ));
        }
        let mut data = Vec::with_capacity(13 + self.value.len());
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.background_color);
        push_u16_le(&mut data, self.font_attributes);
        data.push(self.options);
        push_u16_le(&mut data, self.variable_reference);
        data.push(self.justification);
        push_u16_le(&mut data, self.value.len() as u16);
        data.extend_from_slice(&self.value);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 13 {
            return Err(Error::invalid_data("Output String body too short"));
        }
        if body[7] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Output String body contains reserved option bits",
            ));
        }
        if !text_justification_is_valid(body[10]) {
            return Err(Error::invalid_data(
                "Output String body contains reserved justification",
            ));
        }
        let value_len = u16_le(&body[11..]) as usize;
        if body.len() != 13 + value_len {
            return Err(Error::invalid_data(
                "Output String value length does not match body length",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            background_color: body[4],
            font_attributes: u16_le_as(&body[5..]),
            options: body[7],
            variable_reference: u16_le_as(&body[8..]),
            justification: body[10],
            value: body[13..].to_vec(),
        })
    }
}

/// Output Number body (Type 12). ISO 11783-6 layout: width, height,
/// background, font attributes, options, variable reference, raw value,
/// offset, scale (f32), decimals, format, justification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputNumberBody {
    pub width: u16,
    pub height: u16,
    pub background_color: u8,
    pub font_attributes: ObjectID,
    /// Bit 0 = transparent background, bit 1 = leading zeros, bit 2 =
    /// zero-as-blank, bit 3 = truncate instead of round.
    pub options: u8,
    pub variable_reference: ObjectID,
    /// Raw value used only when `variable_reference` is NULL.
    pub value: u32,
    pub offset: i32,
    /// Scaling factor: `displayed = (value + offset) * scale` (ISO 11783-6
    /// single-precision float). `1.0` = identity.
    pub scale: f32,
    pub number_of_decimals: u8,
    /// 0 = fixed decimal, 1 = exponential.
    pub format: u8,
    /// Bits 0-1 = horizontal justification, bits 2-3 = vertical justification.
    pub justification: u8,
}

impl Default for OutputNumberBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            background_color: 0,
            font_attributes: ObjectID::NULL,
            options: 0,
            variable_reference: ObjectID::NULL,
            value: 0,
            offset: 0,
            scale: 1.0,
            number_of_decimals: 0,
            format: 0,
            justification: 0,
        }
    }
}

impl OutputNumberBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x0F != 0 {
            return Err(Error::invalid_data(
                "Output Number options contain reserved bits",
            ));
        }
        if !self.scale.is_finite() {
            return Err(Error::invalid_data("Output Number scale must be finite"));
        }
        if self.number_of_decimals > 7 {
            return Err(Error::invalid_data(
                "Output Number number of decimals must be in 0..=7",
            ));
        }
        if self.format > 1 {
            return Err(Error::invalid_data("Output Number format must be 0 or 1"));
        }
        if !text_justification_is_valid(self.justification) {
            return Err(Error::invalid_data(
                "Output Number justification contains reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(25);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.background_color);
        push_u16_le(&mut data, self.font_attributes);
        data.push(self.options);
        push_u16_le(&mut data, self.variable_reference);
        data.extend_from_slice(&self.value.to_le_bytes());
        push_i32_le(&mut data, self.offset);
        data.extend_from_slice(&self.scale.to_le_bytes());
        data.push(self.number_of_decimals);
        data.push(self.format);
        data.push(self.justification);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 25 {
            return Err(Error::invalid_data("Output Number body too short"));
        }
        if body[7] & !0x0F != 0 {
            return Err(Error::invalid_data(
                "Output Number body contains reserved option bits",
            ));
        }
        let scale = f32::from_le_bytes([body[18], body[19], body[20], body[21]]);
        if !scale.is_finite() {
            return Err(Error::invalid_data(
                "Output Number body has non-finite scale",
            ));
        }
        if body[22] > 7 {
            return Err(Error::invalid_data(
                "Output Number body contains reserved number of decimals",
            ));
        }
        if body[23] > 1 {
            return Err(Error::invalid_data(
                "Output Number body contains reserved format",
            ));
        }
        if !text_justification_is_valid(body[24]) {
            return Err(Error::invalid_data(
                "Output Number body contains reserved justification",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            background_color: body[4],
            font_attributes: u16_le_as(&body[5..]),
            options: body[7],
            variable_reference: u16_le_as(&body[8..]),
            value: u32::from_le_bytes([body[10], body[11], body[12], body[13]]),
            offset: i32_le(&body[14..]),
            scale,
            number_of_decimals: body[22],
            format: body[23],
            justification: body[24],
        })
    }
}

/// Meter body (Type 17). ISO 11783-6 stores min/max/value as unsigned
/// 16-bit integers (machbus previously used `i32` and omitted `value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterBody {
    pub width: u16,
    pub needle_color: u8,
    pub border_color: u8,
    pub arc_and_tick_color: u8,
    /// Bit 0 = show numeric value.
    pub options: u8,
    pub number_of_ticks: u8,
    pub start_angle: u8,
    pub end_angle: u8,
    pub min_value: u16,
    pub max_value: u16,
    pub variable_reference: ObjectID,
    /// Raw value used only when `variable_reference` is NULL.
    pub value: u16,
}

impl Default for MeterBody {
    fn default() -> Self {
        Self {
            width: 0,
            needle_color: 0,
            border_color: 0,
            arc_and_tick_color: 0,
            options: 0,
            number_of_ticks: 0,
            start_angle: 0,
            end_angle: 0,
            min_value: 0,
            max_value: 0,
            variable_reference: ObjectID::NULL,
            value: 0,
        }
    }
}

impl MeterBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x01 != 0 {
            return Err(Error::invalid_data("Meter options contain reserved bits"));
        }
        validate_half_degree_angle(self.start_angle, "Meter start angle")?;
        validate_half_degree_angle(self.end_angle, "Meter end angle")?;
        let mut data = Vec::with_capacity(17);
        push_u16_le(&mut data, self.width);
        data.push(self.needle_color);
        data.push(self.border_color);
        data.push(self.arc_and_tick_color);
        data.push(self.options);
        data.push(self.number_of_ticks);
        data.push(self.start_angle);
        data.push(self.end_angle);
        push_u16_le(&mut data, self.min_value);
        push_u16_le(&mut data, self.max_value);
        push_u16_le(&mut data, self.variable_reference);
        push_u16_le(&mut data, self.value);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 17 {
            return Err(Error::invalid_data("Meter body too short"));
        }
        if body[5] & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Meter body contains reserved option bits",
            ));
        }
        validate_half_degree_angle(body[7], "Meter body start angle")?;
        validate_half_degree_angle(body[8], "Meter body end angle")?;
        let min_value = u16_le(&body[9..]);
        let max_value = u16_le(&body[11..]);
        Ok(Self {
            width: u16_le(&body[0..]),
            needle_color: body[2],
            border_color: body[3],
            arc_and_tick_color: body[4],
            options: body[5],
            number_of_ticks: body[6],
            start_angle: body[7],
            end_angle: body[8],
            min_value,
            max_value,
            variable_reference: u16_le_as(&body[13..]),
            value: u16_le(&body[15..]),
        })
    }
}

/// Linear Bar Graph body (Type 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinearBarGraphBody {
    pub width: u16,
    pub height: u16,
    pub color: u8,
    pub target_line_color: u8,
    /// Bit 0 = border, bit 1 = target line, bit 2 = ticks, bit 3 = line-only
    /// value marker, bit 4 = horizontal axis, bit 5 = positive direction.
    pub options: u8,
    pub number_of_ticks: u8,
    pub min_value: u16,
    pub max_value: u16,
    pub variable_reference: ObjectID,
    /// Raw value used only when `variable_reference` is NULL.
    pub value: u16,
    pub target_value_variable_reference: ObjectID,
    /// Raw target value used only when `target_value_variable_reference` is NULL.
    pub target_value: u16,
}

impl Default for LinearBarGraphBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            color: 0,
            target_line_color: 0,
            options: 0,
            number_of_ticks: 0,
            min_value: 0,
            max_value: 0,
            variable_reference: ObjectID::NULL,
            value: 0,
            target_value_variable_reference: ObjectID::NULL,
            target_value: 0,
        }
    }
}

impl LinearBarGraphBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x3F != 0 {
            return Err(Error::invalid_data(
                "Linear Bar Graph options contain reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(20);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.color);
        data.push(self.target_line_color);
        data.push(self.options);
        data.push(self.number_of_ticks);
        push_u16_le(&mut data, self.min_value);
        push_u16_le(&mut data, self.max_value);
        push_u16_le(&mut data, self.variable_reference);
        push_u16_le(&mut data, self.value);
        push_u16_le(&mut data, self.target_value_variable_reference);
        push_u16_le(&mut data, self.target_value);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 20 {
            return Err(Error::invalid_data("Linear Bar Graph body too short"));
        }
        if body[6] & !0x3F != 0 {
            return Err(Error::invalid_data(
                "Linear Bar Graph body contains reserved option bits",
            ));
        }
        let min_value = u16_le(&body[8..]);
        let max_value = u16_le(&body[10..]);
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            color: body[4],
            target_line_color: body[5],
            options: body[6],
            number_of_ticks: body[7],
            min_value,
            max_value,
            variable_reference: u16_le_as(&body[12..]),
            value: u16_le(&body[14..]),
            target_value_variable_reference: u16_le_as(&body[16..]),
            target_value: u16_le(&body[18..]),
        })
    }
}

/// Arched Bar Graph body (Type 19). ISO 11783-6 carries a `bar_width`
/// field (not `number_of_ticks`) plus u16 min/max/value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchedBarGraphBody {
    pub width: u16,
    pub height: u16,
    pub color: u8,
    pub target_line_color: u8,
    /// Bit 0 = border, bit 1 = target line, bit 3 = line-only value marker,
    /// bit 4 = clockwise deflection. Bit 2 is undefined by the standard and is
    /// preserved but not rendered.
    pub options: u8,
    pub start_angle: u8,
    pub end_angle: u8,
    /// Bar graph arc width in pixels.
    pub bar_width: u16,
    pub min_value: u16,
    pub max_value: u16,
    pub variable_reference: ObjectID,
    /// Raw value used only when `variable_reference` is NULL.
    pub value: u16,
    pub target_value_variable_reference: ObjectID,
    /// Raw target value used only when `target_value_variable_reference` is NULL.
    pub target_value: u16,
}

impl Default for ArchedBarGraphBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            color: 0,
            target_line_color: 0,
            options: 0,
            start_angle: 0,
            end_angle: 0,
            bar_width: 0,
            min_value: 0,
            max_value: 0,
            variable_reference: ObjectID::NULL,
            value: 0,
            target_value_variable_reference: ObjectID::NULL,
            target_value: 0,
        }
    }
}

impl ArchedBarGraphBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options & !0x1F != 0 {
            return Err(Error::invalid_data(
                "Arched Bar Graph options contain reserved bits",
            ));
        }
        validate_half_degree_angle(self.start_angle, "Arched Bar Graph start angle")?;
        validate_half_degree_angle(self.end_angle, "Arched Bar Graph end angle")?;
        let mut data = Vec::with_capacity(23);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.color);
        data.push(self.target_line_color);
        data.push(self.options);
        data.push(self.start_angle);
        data.push(self.end_angle);
        push_u16_le(&mut data, self.bar_width);
        push_u16_le(&mut data, self.min_value);
        push_u16_le(&mut data, self.max_value);
        push_u16_le(&mut data, self.variable_reference);
        push_u16_le(&mut data, self.value);
        push_u16_le(&mut data, self.target_value_variable_reference);
        push_u16_le(&mut data, self.target_value);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 23 {
            return Err(Error::invalid_data("Arched Bar Graph body too short"));
        }
        if body[6] & !0x1F != 0 {
            return Err(Error::invalid_data(
                "Arched Bar Graph body contains reserved option bits",
            ));
        }
        validate_half_degree_angle(body[7], "Arched Bar Graph body start angle")?;
        validate_half_degree_angle(body[8], "Arched Bar Graph body end angle")?;
        let min_value = u16_le(&body[11..]);
        let max_value = u16_le(&body[13..]);
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            color: body[4],
            target_line_color: body[5],
            options: body[6],
            start_angle: body[7],
            end_angle: body[8],
            bar_width: u16_le(&body[9..]),
            min_value,
            max_value,
            variable_reference: u16_le_as(&body[15..]),
            value: u16_le(&body[17..]),
            target_value_variable_reference: u16_le_as(&body[19..]),
            target_value: u16_le(&body[21..]),
        })
    }
}

/// Picture Graphic body (Type 20). ISO 11783-6 layout: target width,
/// actual width, actual height, format, options, transparency, u32 raw
/// data length, raw data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PictureGraphicBody {
    /// Target display width in pixels.
    pub width: u16,
    /// Actual width of the raw bitmap in pixels.
    pub actual_width: u16,
    /// Actual height of the raw bitmap in pixels.
    pub actual_height: u16,
    /// 0 = 1-bit, 1 = 4-bit, 2 = 8-bit colour-indexed.
    pub format: u8,
    /// Bit 0 = transparent, bit 1 = flashing, bit 2 = run-length encoded.
    /// Bit 2 is static and ignored by Change Attribute at runtime.
    pub options: u8,
    /// Colour index treated as transparent.
    pub transparency: u8,
    pub data: Vec<u8>,
}

impl PictureGraphicBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_picture_graphic_format_and_options(self.format, self.options, "Picture Graphic")?;
        if self.data.len() > u32::MAX as usize {
            return Err(Error::invalid_data(
                "Picture Graphic data exceeds u32 length field",
            ));
        }
        // ISO 11783-6 order: width, actual width, actual height, format,
        // options, transparency, raw-data length (u32), raw data.
        let mut body = Vec::with_capacity(13 + self.data.len());
        push_u16_le(&mut body, self.width);
        push_u16_le(&mut body, self.actual_width);
        push_u16_le(&mut body, self.actual_height);
        body.push(self.format);
        body.push(self.options);
        body.push(self.transparency);
        body.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        body.extend_from_slice(&self.data);
        Ok(body)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 13 {
            return Err(Error::invalid_data("Picture Graphic body too short"));
        }
        validate_picture_graphic_format_and_options(body[6], body[7], "Picture Graphic body")?;
        let data_len = u32::from_le_bytes([body[9], body[10], body[11], body[12]]) as usize;
        if body.len() != 13 + data_len {
            return Err(Error::invalid_data(
                "Picture Graphic data length does not match body length",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            actual_width: u16_le(&body[2..]),
            actual_height: u16_le(&body[4..]),
            format: body[6],
            options: body[7],
            transparency: body[8],
            data: body[13..].to_vec(),
        })
    }
}

/// Return whether a Picture Graphic can be used as a Fill Attributes pattern
/// without leaving unused trailing bits in each raw-data row.
///
/// ISO 11783-6 requires pattern Picture Graphics to avoid unused row bits for
/// monochrome and 16-colour formats. The check is about the raw bitmap width,
/// not the target display width used when drawing the picture as an object.
#[must_use]
pub fn picture_graphic_fill_pattern_buffer_is_valid(body: &PictureGraphicBody) -> bool {
    match body.format {
        // 1-bit rows must end on a whole byte.
        0 => body.actual_width.is_multiple_of(8),
        // 4-bit rows must end on a whole byte.
        1 => body.actual_width.is_multiple_of(2),
        // 8-bit indexed rows always end on a byte.
        2 => true,
        // Reserved formats are rejected by PictureGraphicBody::decode/encode;
        // treat them as invalid here for callers that hold a hand-built body.
        _ => false,
    }
}

/// Working Set body (Type 0). ISO 11783-6 fixed fields are followed in the
/// serialized object record by object-count, macro-count, language-count, child
/// location records, macro references, and two-byte language codes. The child
/// and macro lists live on [`VTObject`]; language codes are kept here because
/// they are Working-Set metadata rather than child objects.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkingSetBody {
    pub background_colour: u8,
    /// 0 = not selectable, 1 = selectable.
    pub selectable: u8,
    /// Object ID of the initial Data Mask or Alarm Mask.
    pub active_mask: ObjectID,
    /// Two-byte ISO 639 language codes advertised by this Working Set.
    pub languages: Vec<[u8; 2]>,
}

impl WorkingSetBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(4 + self.languages.len().saturating_mul(2));
        data.push(self.background_colour);
        data.push(self.selectable);
        push_u16_le(&mut data, self.active_mask);
        for language in &self.languages {
            data.extend_from_slice(language);
        }
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(Error::invalid_data("Working Set fixed body too short"));
        }
        let language_bytes = body.len() - 4;
        if !language_bytes.is_multiple_of(2) {
            return Err(Error::invalid_data(
                "Working Set language-code list must contain two-byte codes",
            ));
        }
        let mut languages = Vec::with_capacity(language_bytes / 2);
        for chunk in body[4..].chunks_exact(2) {
            languages.push([chunk[0], chunk[1]]);
        }
        Ok(Self {
            background_colour: body[0],
            selectable: body[1],
            active_mask: u16_le_as(&body[2..]),
            languages,
        })
    }
}

/// Auxiliary Function body (Type 29).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxFunctionBody {
    pub function_type: u8,
    /// Bits 0..=1 are used by the classic AUX function capability flags.
    pub options: u8,
    pub designator: ObjectID,
}

impl Default for AuxFunctionBody {
    fn default() -> Self {
        Self {
            function_type: 0,
            options: 0,
            designator: ObjectID::NULL,
        }
    }
}

impl AuxFunctionBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.function_type > 2 {
            return Err(Error::invalid_data("Aux Function type must be in 0..=2"));
        }
        if self.options & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Aux Function options contain reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(4);
        data.push(self.function_type);
        data.push(self.options);
        push_u16_le(&mut data, self.designator);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(Error::invalid_data("Aux Function body too short"));
        }
        if body[0] > 2 {
            return Err(Error::invalid_data(
                "Aux Function body contains reserved type",
            ));
        }
        if body[1] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Aux Function body contains reserved option bits",
            ));
        }
        Ok(Self {
            function_type: body[0],
            options: body[1],
            designator: u16_le_as(&body[2..]),
        })
    }
}

/// Auxiliary Input body (Type 30).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxInputBody {
    pub input_type: u8,
    pub input_id: u8,
    /// Bit 0 = enabled.
    pub options: u8,
}

impl AuxInputBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.input_type > 2 {
            return Err(Error::invalid_data("Aux Input type must be in 0..=2"));
        }
        if self.options & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Aux Input options contain reserved bits",
            ));
        }
        Ok(vec![self.input_type, self.input_id, self.options])
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 3 {
            return Err(Error::invalid_data("Aux Input body too short"));
        }
        if body[0] > 2 {
            return Err(Error::invalid_data("Aux Input body contains reserved type"));
        }
        if body[2] & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Aux Input body contains reserved option bits",
            ));
        }
        Ok(Self {
            input_type: body[0],
            input_id: body[1],
            options: body[2],
        })
    }
}

/// Auxiliary Function 2 body (Type 31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxFunction2Body {
    pub function_type: u8,
    pub function_attributes: u8,
    pub name: ObjectID,
    pub icon: ObjectID,
}

impl Default for AuxFunction2Body {
    fn default() -> Self {
        Self {
            function_type: 0,
            function_attributes: 0,
            name: ObjectID::NULL,
            icon: ObjectID::NULL,
        }
    }
}

impl AuxFunction2Body {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.function_type > 2 {
            return Err(Error::invalid_data("Aux Function 2 type must be in 0..=2"));
        }
        let mut data = Vec::with_capacity(6);
        data.push(self.function_type);
        data.push(self.function_attributes);
        push_u16_le(&mut data, self.name);
        push_u16_le(&mut data, self.icon);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 6 {
            return Err(Error::invalid_data("Aux Function 2 body too short"));
        }
        if body[0] > 2 {
            return Err(Error::invalid_data(
                "Aux Function 2 body contains reserved type",
            ));
        }
        Ok(Self {
            function_type: body[0],
            function_attributes: body[1],
            name: u16_le_as(&body[2..]),
            icon: u16_le_as(&body[4..]),
        })
    }
}

/// Auxiliary Input 2 body (Type 32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxInput2Body {
    pub input_type: u8,
    pub input_id: u8,
    pub input_status: u8,
    pub input_value: u16,
    pub name: ObjectID,
}

impl Default for AuxInput2Body {
    fn default() -> Self {
        Self {
            input_type: 0,
            input_id: 0,
            input_status: 0,
            input_value: 0,
            name: ObjectID::NULL,
        }
    }
}

impl AuxInput2Body {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.input_type > 2 {
            return Err(Error::invalid_data("Aux Input 2 type must be in 0..=2"));
        }
        if self.input_status > 3 {
            return Err(Error::invalid_data("Aux Input 2 status must be in 0..=3"));
        }
        let mut data = Vec::with_capacity(7);
        data.push(self.input_type);
        data.push(self.input_id);
        data.push(self.input_status);
        push_u16_le(&mut data, self.input_value);
        push_u16_le(&mut data, self.name);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 7 {
            return Err(Error::invalid_data("Aux Input 2 body too short"));
        }
        if body[0] > 2 {
            return Err(Error::invalid_data(
                "Aux Input 2 body contains reserved type",
            ));
        }
        if body[2] > 3 {
            return Err(Error::invalid_data(
                "Aux Input 2 body contains reserved status",
            ));
        }
        Ok(Self {
            input_type: body[0],
            input_id: body[1],
            input_status: body[2],
            input_value: u16_le(&body[3..]),
            name: u16_le_as(&body[5..]),
        })
    }
}

/// Auxiliary Control Designator body (Type 33).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxControlDesignatorBody {
    pub aux_object: ObjectID,
    pub designator: Vec<u8>,
}

impl Default for AuxControlDesignatorBody {
    fn default() -> Self {
        Self {
            aux_object: ObjectID::NULL,
            designator: Vec::new(),
        }
    }
}

impl AuxControlDesignatorBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.designator.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Aux Control Designator text exceeds u8 length field",
            ));
        }
        let mut data = Vec::with_capacity(3 + self.designator.len());
        push_u16_le(&mut data, self.aux_object);
        data.push(self.designator.len() as u8);
        data.extend_from_slice(&self.designator);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 3 {
            return Err(Error::invalid_data("Aux Control Designator body too short"));
        }
        let len = body[2] as usize;
        if body.len() != 3 + len {
            return Err(Error::invalid_data(
                "Aux Control Designator text length does not match body length",
            ));
        }
        Ok(Self {
            aux_object: u16_le_as(&body[0..]),
            designator: body[3..].to_vec(),
        })
    }
}

/// Graphic Data body (Type 46).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GraphicDataBody {
    /// Standard graphic format. ISO 11783-6:2018 defines only
    /// `0 = PNG, restricted to 32-bit RGBA maximum` for Graphic Data.
    pub format: u8,
    /// Reserved compatibility field kept so existing callers can still build
    /// the struct. Standard Graphic Data has no options byte and this must be
    /// zero when encoded.
    pub options: u8,
    pub data: Vec<u8>,
}

impl GraphicDataBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.format != 0 {
            return Err(Error::invalid_data("Graphic Data format must be PNG (0)"));
        }
        if self.options != 0 {
            return Err(Error::invalid_data(
                "Graphic Data has no standard options byte",
            ));
        }
        if self.data.len() > u32::MAX as usize {
            return Err(Error::invalid_data(
                "Graphic Data payload exceeds u32 length field",
            ));
        }
        let mut data = Vec::with_capacity(5 + self.data.len());
        data.push(self.format);
        push_u32_le(&mut data, self.data.len() as u32);
        data.extend_from_slice(&self.data);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 5 {
            return Err(Error::invalid_data("Graphic Data body too short"));
        }
        if body[0] != 0 {
            return Err(Error::invalid_data(
                "Graphic Data body format must be PNG (0)",
            ));
        }
        let len = u32_le(&body[1..]) as usize;
        if body.len() != 5 + len {
            return Err(Error::invalid_data(
                "Graphic Data payload length does not match body length",
            ));
        }
        Ok(Self {
            format: body[0],
            options: 0,
            data: body[5..].to_vec(),
        })
    }
}

/// Scaled Graphic body (Type 48).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaledGraphicBody {
    pub width: u16,
    pub height: u16,
    /// Standard ScaleType byte:
    /// bits 0-2 = scaling mode, bits 3-4 = horizontal justification,
    /// bits 5-6 = vertical justification, bit 7 reserved.
    pub scale_type: u8,
    /// Bit 0 = flashing; bits 1-7 reserved.
    pub options: u8,
    /// Value attribute: Graphic Data, Picture Graphic, Object Pointer, or NULL.
    pub value: ObjectID,
}

impl Default for ScaledGraphicBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            scale_type: 0,
            options: 0,
            value: ObjectID::NULL,
        }
    }
}

impl ScaledGraphicBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if !scaled_graphic_scale_type_is_valid(self.scale_type) {
            return Err(Error::invalid_data(
                "Scaled Graphic ScaleType contains reserved values",
            ));
        }
        if self.options & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Scaled Graphic options contain reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(8);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.scale_type);
        data.push(self.options);
        push_u16_le(&mut data, self.value);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 8 {
            return Err(Error::invalid_data("Scaled Graphic body too short"));
        }
        if !scaled_graphic_scale_type_is_valid(body[4]) {
            return Err(Error::invalid_data(
                "Scaled Graphic body contains reserved ScaleType values",
            ));
        }
        if body[5] & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Scaled Graphic body contains reserved option bits",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            scale_type: body[4],
            options: body[5],
            value: u16_le_as(&body[6..]),
        })
    }
}

#[must_use]
pub(crate) const fn scaled_graphic_scale_type_is_valid(scale_type: u8) -> bool {
    let scaling_value = scale_type & 0x07;
    let horizontal_justification = (scale_type >> 3) & 0x03;
    let vertical_justification = (scale_type >> 5) & 0x03;
    scaling_value <= 4 && horizontal_justification <= 2 && vertical_justification <= 2
}

/// Animation body (Type 44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationBody {
    pub width: u16,
    pub height: u16,
    pub refresh_interval_ms: u16,
    /// List index of the child object to show; `255` means no item is chosen.
    pub value: u8,
    /// `0` = stopped, `1` = animating.
    pub enabled: u8,
    /// First child index in the animation sequence.
    pub first_child_index: u8,
    /// Default child index used by disabled-behaviour option `2`.
    pub default_child_index: u8,
    /// Last child index in the animation sequence.
    pub last_child_index: u8,
    /// Bit 0 = loop; bits 1-2 = disabled behaviour; bits 3-7 reserved.
    pub options: u8,
}

impl AnimationBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.enabled > 1 {
            return Err(Error::invalid_data("Animation enabled must be 0 or 1"));
        }
        if self.options & !0x07 != 0 {
            return Err(Error::invalid_data(
                "Animation options contain reserved bits",
            ));
        }
        let mut data = Vec::with_capacity(12);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        push_u16_le(&mut data, self.refresh_interval_ms);
        data.push(self.value);
        data.push(self.enabled);
        data.push(self.first_child_index);
        data.push(self.default_child_index);
        data.push(self.last_child_index);
        data.push(self.options);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 12 {
            return Err(Error::invalid_data("Animation body too short"));
        }
        if body[7] > 1 {
            return Err(Error::invalid_data(
                "Animation body contains invalid enabled value",
            ));
        }
        if body[11] & !0x07 != 0 {
            return Err(Error::invalid_data(
                "Animation body contains reserved option bits",
            ));
        }
        if body.len() != 12 {
            return Err(Error::invalid_data("Animation fixed body length mismatch"));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            refresh_interval_ms: u16_le(&body[4..]),
            value: body[6],
            enabled: body[7],
            first_child_index: body[8],
            default_child_index: body[9],
            last_child_index: body[10],
            options: body[11],
        })
    }
}

/// Colour Map body (Type 39).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColourMapBody {
    pub entries: Vec<u8>,
}
