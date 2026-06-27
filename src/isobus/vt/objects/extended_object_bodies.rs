impl Default for WorkingSetSpecialControlsBody {
    fn default() -> Self {
        Self {
            colour_map: ObjectID::NULL,
            colour_palette: ObjectID::NULL,
            languages: Vec::new(),
            extra_bytes: Vec::new(),
        }
    }
}

impl WorkingSetSpecialControlsBody {
    pub fn bytes_to_follow(&self) -> Result<u16> {
        let language_bytes = self.languages.len().checked_mul(4).ok_or_else(|| {
            Error::invalid_data("Working Set Special Controls language length overflows")
        })?;
        let bytes_to_follow = 5usize
            .checked_add(language_bytes)
            .and_then(|value| value.checked_add(self.extra_bytes.len()))
            .ok_or_else(|| {
                Error::invalid_data("Working Set Special Controls byte count overflows")
            })?;
        u16::try_from(bytes_to_follow).map_err(|_| {
            Error::invalid_data("Working Set Special Controls byte count exceeds u16 field")
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.languages.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Working Set Special Controls language count exceeds u8 count field",
            ));
        }
        let bytes_to_follow = self.bytes_to_follow()? as usize;
        let mut data = Vec::with_capacity(2 + bytes_to_follow);
        push_u16_le(&mut data, bytes_to_follow as u16);
        push_u16_le(&mut data, self.colour_map);
        push_u16_le(&mut data, self.colour_palette);
        data.push(self.languages.len() as u8);
        for pair in &self.languages {
            data.extend_from_slice(&pair.language);
            data.extend_from_slice(&pair.country);
        }
        data.extend_from_slice(&self.extra_bytes);
        Ok(data)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 2 {
            return Err(Error::invalid_data(
                "Working Set Special Controls body too short",
            ));
        }
        let bytes_to_follow = u16_le(data) as usize;
        if data.len() != 2 + bytes_to_follow {
            return Err(Error::invalid_data(
                "Working Set Special Controls byte count does not match body length",
            ));
        }
        let mut colour_map = ObjectID::NULL;
        let mut colour_palette = ObjectID::NULL;
        let mut languages = Vec::new();
        let mut extra_bytes = Vec::new();
        if bytes_to_follow >= 2 {
            colour_map = u16_le_as(&data[2..]);
        }
        if bytes_to_follow >= 4 {
            colour_palette = u16_le_as(&data[4..]);
        }
        if bytes_to_follow >= 5 {
            let count = data[6] as usize;
            let pairs_bytes = count.checked_mul(4).ok_or_else(|| {
                Error::invalid_data("Working Set Special Controls language length overflows")
            })?;
            let expected = 5usize.checked_add(pairs_bytes).ok_or_else(|| {
                Error::invalid_data("Working Set Special Controls language length overflows")
            })?;
            if expected > bytes_to_follow {
                return Err(Error::invalid_data(
                    "Working Set Special Controls language count exceeds body length",
                ));
            }
            let mut offset = 7;
            for _ in 0..count {
                languages.push(LanguageCountryPair {
                    language: [data[offset], data[offset + 1]],
                    country: [data[offset + 2], data[offset + 3]],
                });
                offset += 4;
            }
            extra_bytes.extend_from_slice(&data[2 + expected..2 + bytes_to_follow]);
        }
        Ok(Self {
            colour_map,
            colour_palette,
            languages,
            extra_bytes,
        })
    }
}

/// Graphics Context extension body (machbus compatibility type 50).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GraphicsContextBody {
    pub context: GraphicsContextV6,
}

impl GraphicsContextBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.context.encode()
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let context = GraphicsContextV6::decode(data)
            .ok_or_else(|| Error::invalid_data("Graphics Context body is invalid"))?;
        Ok(Self { context })
    }
}

/// One Object Label Reference List entry (Type 40, VT4+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectLabelRefEntry {
    pub labelled_object: ObjectID,
    pub string_variable: ObjectID,
    pub font_type: u8,
    pub graphic_designator: ObjectID,
}

/// Object Label Reference List body (Type 40, VT4+).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectLabelRefBody {
    pub labels: Vec<ObjectLabelRefEntry>,
}

impl ObjectLabelRefBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let count = u16::try_from(self.labels.len()).unwrap_or(u16::MAX);
        let mut data = Vec::with_capacity(2 + usize::from(count) * 7);
        push_u16_le(&mut data, count);
        for label in self.labels.iter().take(usize::from(count)) {
            push_u16_le(&mut data, label.labelled_object);
            push_u16_le(&mut data, label.string_variable);
            data.push(label.font_type);
            push_u16_le(&mut data, label.graphic_designator);
        }
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 2 {
            return Err(Error::invalid_data("Object Label Reference body too short"));
        }
        let count = u16_le(&data[0..]) as usize;
        let expected = 2usize
            .checked_add(count.checked_mul(7).ok_or_else(|| {
                Error::invalid_data("Object Label Reference body length overflows")
            })?)
            .ok_or_else(|| Error::invalid_data("Object Label Reference body length overflows"))?;
        if data.len() != expected {
            return Err(Error::invalid_data(
                "Object Label Reference body length does not match labelled-object count",
            ));
        }
        let mut labels = Vec::with_capacity(count);
        let mut cursor = 2usize;
        for _ in 0..count {
            let labelled_object = u16_le_as(&data[cursor..]);
            cursor += 2;
            let string_variable = u16_le_as(&data[cursor..]);
            cursor += 2;
            let font_type = data[cursor];
            cursor += 1;
            let graphic_designator = u16_le_as(&data[cursor..]);
            cursor += 2;
            labels.push(ObjectLabelRefEntry {
                labelled_object,
                string_variable,
                font_type,
                graphic_designator,
            });
        }
        Ok(Self { labels })
    }
}

// ─── Scaled bitmap (VT6) ──────────────────────────────────────────────

/// Scaled bitmap extension body (machbus compatibility type 49). `scale_x` / `scale_y` use 8.8
/// fixed-point on the wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaledBitmapBody {
    pub width: u16,
    pub height: u16,
    pub scale_x: f32,
    pub scale_y: f32,
    pub offset_x: i16,
    pub offset_y: i16,
    /// 0 = 1bit, 1 = 4bit, 2 = 8bit, 3 = 24bit RGB.
    pub format: u8,
    /// bit 0 = RLE compressed.
    pub options: u8,
    pub bitmap_data: ObjectID,
}

impl Default for ScaledBitmapBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            scale_x: 1.0,
            scale_y: 1.0,
            offset_x: 0,
            offset_y: 0,
            format: 0,
            options: 0,
            bitmap_data: ObjectID::NULL,
        }
    }
}

impl ScaledBitmapBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.format > 3 {
            return Err(Error::invalid_data(
                "Scaled Bitmap format must fit the VT6 0..=3 bitmap format field",
            ));
        }
        if self.options & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Scaled Bitmap options contain reserved bits",
            ));
        }

        let mut data = Vec::with_capacity(16);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        push_u16_le(&mut data, encode_u8_8_scale(self.scale_x, "scale_x")?);
        push_u16_le(&mut data, encode_u8_8_scale(self.scale_y, "scale_y")?);
        push_u16_le(&mut data, self.offset_x as u16);
        push_u16_le(&mut data, self.offset_y as u16);
        data.push(self.format);
        data.push(self.options);
        push_u16_le(&mut data, self.bitmap_data);
        Ok(data)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 16 {
            return Err(Error::invalid_data("Scaled Bitmap body too short"));
        }
        if data[12] > 3 {
            return Err(Error::invalid_data(
                "Scaled Bitmap body contains invalid bitmap format",
            ));
        }
        if data[13] & !0x01 != 0 {
            return Err(Error::invalid_data(
                "Scaled Bitmap body contains reserved option bits",
            ));
        }
        Ok(Self {
            width: u16_le(&data[0..]),
            height: u16_le(&data[2..]),
            scale_x: u16_le(&data[4..]) as f32 / 256.0,
            scale_y: u16_le(&data[6..]) as f32 / 256.0,
            offset_x: u16_le(&data[8..]) as i16,
            offset_y: u16_le(&data[10..]) as i16,
            format: data[12],
            options: data[13],
            bitmap_data: u16_le_as(&data[14..]),
        })
    }
}

fn encode_u8_8_scale(value: f32, field: &'static str) -> Result<u16> {
    if !value.is_finite() {
        return Err(Error::invalid_data(format!(
            "Scaled Bitmap {field} must be finite"
        )));
    }
    let raw = trunc_f32(value * 256.0);
    if !(0.0..=u16::MAX as f32).contains(&raw) {
        return Err(Error::invalid_data(format!(
            "Scaled Bitmap {field} exceeds u8.8 wire range"
        )));
    }
    Ok(raw as u16)
}

fn trunc_f32(value: f32) -> f32 {
    value as i64 as f32
}

// ─── Colour palette (VT6) ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColourPaletteEntry {
    /// 24-bit RGB color.
    pub rgb: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColourPalette {
    pub entries: Vec<ColourPaletteEntry>,
}

impl ColourPalette {
    pub fn add_color(&mut self, rgb: u32, name: impl Into<String>) {
        self.entries.push(ColourPaletteEntry {
            rgb,
            name: name.into(),
        });
    }

    /// VT6 standard 16-entry palette.
    #[must_use]
    pub fn create_standard_v6() -> Self {
        let mut p = Self::default();
        p.add_color(0x00_00_00, "black");
        p.add_color(0xFF_FF_FF, "white");
        p.add_color(0xFF_00_00, "red");
        p.add_color(0x00_FF_00, "green");
        p.add_color(0x00_00_FF, "blue");
        p.add_color(0xFF_FF_00, "yellow");
        p.add_color(0xFF_00_FF, "magenta");
        p.add_color(0x00_FF_FF, "cyan");
        p.add_color(0x80_80_80, "gray");
        p.add_color(0xFF_A5_00, "orange");
        p.add_color(0x80_00_80, "purple");
        p.add_color(0xA5_2A_2A, "brown");
        p.add_color(0xFF_C0_CB, "pink");
        p.add_color(0x00_FF_7F, "spring green");
        p.add_color(0x4B_00_82, "indigo");
        p.add_color(0xFF_63_47, "tomato");
        p
    }
}

// ─── helpers ──────────────────────────────────────────────────────────

#[inline]
fn u16_le(buf: &[u8]) -> u16 {
    (buf[0] as u16) | ((buf[1] as u16) << 8)
}

#[inline]
fn u32_le(buf: &[u8]) -> u32 {
    (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24)
}

#[inline]
fn i32_le(buf: &[u8]) -> i32 {
    u32_le(buf) as i32
}

/// Read a little-endian `u16` and convert into any `T: From<u16>`
/// (e.g. an [`ObjectID`] newtype).
#[inline]
fn u16_le_as<T: From<u16>>(buf: &[u8]) -> T {
    u16_le(buf).into()
}

#[inline]
fn push_u16_le<T: Into<u16>>(out: &mut Vec<u8>, v: T) {
    let v = v.into();
    out.push((v & 0xFF) as u8);
    out.push(((v >> 8) & 0xFF) as u8);
}

#[inline]
fn push_u32_le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

#[inline]
fn push_i32_le(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn validate_half_degree_angle(value: u8, field: &'static str) -> Result<()> {
    if value > 180 {
        return Err(Error::invalid_data(format!(
            "{field} must be a half-degree value in 0..=180"
        )));
    }
    Ok(())
}

fn validate_picture_graphic_format_and_options(
    format: u8,
    options: u8,
    object: &'static str,
) -> Result<()> {
    if format > 2 {
        return Err(Error::invalid_data(format!(
            "{object} format must fit the 0..=2 indexed bitmap format field"
        )));
    }
    if options & !0x07 != 0 {
        return Err(Error::invalid_data(format!(
            "{object} options contain reserved bits"
        )));
    }
    Ok(())
}

fn serialized_child_macro_tail_len(
    child_count: usize,
    macro_count: usize,
    record_size: usize,
) -> Result<usize> {
    if child_count > u8::MAX as usize || macro_count > u8::MAX as usize {
        return Err(Error::invalid_data(
            "VT object child/macro count exceeds u8 count field",
        ));
    }
    // `[num_objects:u8] + child_count*record_size + [num_macros:u8] + macro_count*2`
    let child_bytes = child_count
        .checked_mul(record_size)
        .ok_or_else(|| Error::invalid_data("VT object child-list length overflows"))?;
    let macro_bytes = macro_count
        .checked_mul(2)
        .ok_or_else(|| Error::invalid_data("VT object macro-list length overflows"))?;
    1usize
        .checked_add(child_bytes)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_add(macro_bytes))
        .ok_or_else(|| Error::invalid_data("VT object child/macro tail length overflows"))
}

#[inline]
fn push_u24_le(out: &mut Vec<u8>, v: u32) {
    out.push((v & 0xFF) as u8);
    out.push(((v >> 8) & 0xFF) as u8);
    out.push(((v >> 16) & 0xFF) as u8);
}

fn split_body_and_children(
    r#type: ObjectType,
    raw_body: &[u8],
) -> Result<(Vec<u8>, Vec<ChildRef>, Vec<MacroRef>)> {
    if r#type == ObjectType::WorkingSet {
        return split_working_set_body(raw_body).ok_or_else(|| {
            Error::with_message(
                ErrorCode::PoolValidation,
                "malformed Working Set child/macro/language tail",
            )
        });
    }
    if r#type == ObjectType::PictureGraphic {
        return split_picture_graphic(raw_body).ok_or_else(|| {
            Error::with_message(
                ErrorCode::PoolValidation,
                "malformed Picture Graphic macro tail",
            )
        });
    }
    let Some((child_list_offset, record_size)) = parent_layout_for_body(r#type, raw_body)? else {
        // Leaf object: a per-type body, optionally followed by the standard
        // `[num_macros][macro refs]` tail (no child list).
        if leaf_has_macro_tail(r#type) {
            let body_len = leaf_body_only_len(r#type, raw_body, 0)?;
            return split_leaf_macro_tail(raw_body, body_len).ok_or_else(|| {
                Error::with_message(ErrorCode::PoolValidation, "malformed leaf macro tail")
            });
        }
        return Ok((raw_body.to_vec(), Vec::new(), Vec::new()));
    };

    if raw_body.len() <= child_list_offset {
        return Ok((raw_body.to_vec(), Vec::new(), Vec::new()));
    }
    split_child_list_at(raw_body, child_list_offset, record_size).ok_or_else(|| {
        Error::with_message(
            ErrorCode::PoolValidation,
            format!(
                "malformed child/macro tail for VT object type {type_id}",
                type_id = r#type.as_u8()
            ),
        )
    })
}

fn split_working_set_body(raw_body: &[u8]) -> Option<(Vec<u8>, Vec<ChildRef>, Vec<MacroRef>)> {
    // ISO 11783-6 Working Set tail:
    //   fixed[4], object_count, macro_count, language_count,
    //   child records (oid,x,y), macro records (event,macro), language codes.
    if raw_body.len() < 7 {
        return None;
    }
    let child_count = raw_body[4] as usize;
    if child_count == 0 {
        return None;
    }
    let macro_count = raw_body[5] as usize;
    let language_count = raw_body[6] as usize;
    let child_bytes = child_count.checked_mul(6)?;
    let macro_bytes = macro_count.checked_mul(2)?;
    let language_bytes = language_count.checked_mul(2)?;
    let children_offset = 7usize;
    let macros_offset = children_offset.checked_add(child_bytes)?;
    let languages_offset = macros_offset.checked_add(macro_bytes)?;
    let total = languages_offset.checked_add(language_bytes)?;
    if total != raw_body.len() {
        return None;
    }

    let mut children = Vec::with_capacity(child_count);
    let mut offset = children_offset;
    for _ in 0..child_count {
        let id = u16_le_as(&raw_body[offset..]);
        let x = i16::from_le_bytes([raw_body[offset + 2], raw_body[offset + 3]]);
        let y = i16::from_le_bytes([raw_body[offset + 4], raw_body[offset + 5]]);
        children.push(ChildRef::new(id, x, y));
        offset += 6;
    }
    let mut macros = Vec::with_capacity(macro_count);
    offset = macros_offset;
    for _ in 0..macro_count {
        macros.push(MacroRef::new(raw_body[offset], raw_body[offset + 1]));
        offset += 2;
    }

    let mut body = Vec::with_capacity(4 + language_bytes);
    body.extend_from_slice(&raw_body[..4]);
    body.extend_from_slice(&raw_body[languages_offset..]);
    Some((body, children, macros))
}

#[must_use]
fn split_child_list_at(
    raw_body: &[u8],
    child_list_offset: usize,
    record_size: usize,
) -> Option<(Vec<u8>, Vec<ChildRef>, Vec<MacroRef>)> {
    // ISO 11783-6 parent tail — both counts precede both lists:
    //   `[num_objects:u8][num_macros:u8][num_objects × record][num_macros × 2]`
    // where `record` is 6 bytes (`oid:u16, x:i16, y:i16`) for positional
    // parents and 2 bytes (`oid:u16`) for OID-only parents (Soft Key Mask,
    // Key Group, Input List). Each macro ref is `[event:u8, macro:u8]`.
    if raw_body.len().saturating_sub(child_list_offset) < 2 {
        return None;
    }
    let child_count = raw_body[child_list_offset] as usize;
    let macro_count = raw_body[child_list_offset + 1] as usize;
    let child_bytes = child_count.checked_mul(record_size)?;
    let macro_bytes = macro_count.checked_mul(2)?;
    let total_tail = 2usize.checked_add(child_bytes)?.checked_add(macro_bytes)?;
    if child_list_offset.checked_add(total_tail)? != raw_body.len() {
        return None;
    }

    let mut children = Vec::with_capacity(child_count);
    // Children begin after both count bytes (num_objects + num_macros).
    let mut offset = child_list_offset + 2;
    for _ in 0..child_count {
        let id = u16_le_as(&raw_body[offset..]);
        let (x, y) = if record_size == 6 {
            let x = i16::from_le_bytes([raw_body[offset + 2], raw_body[offset + 3]]);
            let y = i16::from_le_bytes([raw_body[offset + 4], raw_body[offset + 5]]);
            (x, y)
        } else {
            (0, 0)
        };
        children.push(ChildRef::new(id, x, y));
        offset += record_size;
    }
    let mut macros = Vec::with_capacity(macro_count);
    for _ in 0..macro_count {
        macros.push(MacroRef::new(raw_body[offset], raw_body[offset + 1]));
        offset += 2;
    }
    Some((raw_body[..child_list_offset].to_vec(), children, macros))
}

/// Split a Picture Graphic body. On the wire it is
/// `[13 header incl data-length][num_macros][raw data][macro refs]`; the stored
/// body keeps `[13 header][raw data]` (matching the body codec) and the macro
/// refs are returned separately. Returns `None` on a malformed tail.
fn split_picture_graphic(raw_body: &[u8]) -> Option<(Vec<u8>, Vec<ChildRef>, Vec<MacroRef>)> {
    if raw_body.len() < 14 {
        return None;
    }
    let data_len =
        u32::from_le_bytes([raw_body[9], raw_body[10], raw_body[11], raw_body[12]]) as usize;
    let num_macros = raw_body[13] as usize;
    let data_end = 14usize.checked_add(data_len)?;
    let macro_bytes = num_macros.checked_mul(2)?;
    if data_end.checked_add(macro_bytes)? != raw_body.len() {
        return None;
    }
    let mut body = Vec::with_capacity(13 + data_len);
    body.extend_from_slice(&raw_body[..13]);
    body.extend_from_slice(&raw_body[14..data_end]);
    let mut macros = Vec::with_capacity(num_macros);
    let mut o = data_end;
    for _ in 0..num_macros {
        macros.push(MacroRef::new(raw_body[o], raw_body[o + 1]));
        o += 2;
    }
    Some((body, Vec::new(), macros))
}

/// Split a leaf object body into `(body, [], macros)`: the first `body_len`
/// bytes are the object body, and the remainder is the standard trailing
/// `[num_macros][macro refs]` list. Returns `None` on a malformed tail.
fn split_leaf_macro_tail(
    raw_body: &[u8],
    body_len: usize,
) -> Option<(Vec<u8>, Vec<ChildRef>, Vec<MacroRef>)> {
    let macro_count = *raw_body.get(body_len)? as usize;
    let macro_bytes = macro_count.checked_mul(2)?;
    if body_len.checked_add(1)?.checked_add(macro_bytes)? != raw_body.len() {
        return None;
    }
    let mut macros = Vec::with_capacity(macro_count);
    let mut o = body_len + 1;
    for _ in 0..macro_count {
        macros.push(MacroRef::new(raw_body[o], raw_body[o + 1]));
        o += 2;
    }
    Some((raw_body[..body_len].to_vec(), Vec::new(), macros))
}

/// `(child_list_offset, record_size)` for parent types. `record_size` is
/// 6 for positional parents (OID + X + Y) and 2 for OID-only parents
/// (Soft Key Mask, Key Group). Returns `None` for leaf types.
#[inline]
#[must_use]
const fn parent_layout(r#type: ObjectType) -> Option<(usize, usize)> {
    match r#type {
        // Positional parents (6-byte child records).
        ObjectType::WorkingSet => Some((4, 6)),
        ObjectType::DataMask => Some((3, 6)),
        ObjectType::AlarmMask => Some((5, 6)),
        ObjectType::Container => Some((5, 6)),
        ObjectType::Key => Some((2, 6)),
        ObjectType::Button => Some((8, 6)),
        ObjectType::Animation => Some((12, 6)),
        // OID-only parents (2-byte child records).
        ObjectType::SoftKeyMask => Some((1, 2)),
        ObjectType::KeyGroup => Some((5, 2)),
        _ => None,
    }
}

fn parent_layout_for_body(r#type: ObjectType, raw_body: &[u8]) -> Result<Option<(usize, usize)>> {
    if r#type == ObjectType::WindowMask {
        return Ok(Some((window_mask_child_list_offset(raw_body, 0)?, 6)));
    }
    Ok(parent_layout(r#type))
}

fn window_mask_child_list_offset(data: &[u8], off: usize) -> Result<usize> {
    let required_count =
        data.get(off + 11)
            .copied()
            .ok_or_else(|| Error::invalid_data("Window Mask body too short"))? as usize;
    let required_bytes = required_count
        .checked_mul(2)
        .ok_or_else(|| Error::invalid_data("Window Mask required-object list overflows"))?;
    12usize
        .checked_add(required_bytes)
        .ok_or_else(|| Error::invalid_data("Window Mask child-list offset overflows"))
}

#[inline]
#[must_use]
const fn object_type_may_have_children(r#type: ObjectType) -> bool {
    match r#type {
        ObjectType::WindowMask => true,
        _ => parent_layout(r#type).is_some(),
    }
}

#[inline]
#[must_use]
const fn is_ascii_letter_pair(pair: [u8; 2]) -> bool {
    pair[0].is_ascii_alphabetic() && pair[1].is_ascii_alphabetic()
}

const fn is_not_applicable_country_pair(pair: [u8; 2]) -> bool {
    pair[0] == b' ' && pair[1] == b' '
}

#[inline]
fn pool_walk_short() -> Error {
    Error::with_message(
        ErrorCode::PoolValidation,
        "object body extends past pool data",
    )
}

/// Read a length/count field from `data` at absolute offset `off`,
/// erroring if it would read past the buffer.
#[inline]
fn peek_u8_at(data: &[u8], off: usize) -> Result<usize> {
    data.get(off)
        .map(|&b| b as usize)
        .ok_or_else(pool_walk_short)
}

#[inline]
fn peek_u16_at(data: &[u8], off: usize) -> Result<usize> {
    if off + 2 > data.len() {
        return Err(pool_walk_short());
    }
    Ok(u16_le(&data[off..]) as usize)
}

#[inline]
fn peek_u32_at(data: &[u8], off: usize) -> Result<usize> {
    if off + 4 > data.len() {
        return Err(pool_walk_short());
    }
    Ok(u32_le(&data[off..]) as usize)
}

fn extended_input_attributes_len(data: &[u8], off: usize) -> Result<usize> {
    if off + 2 > data.len() {
        return Err(pool_walk_short());
    }
    let plane_count = data[off + 1] as usize;
    if plane_count == 0 || plane_count > 17 {
        return Err(Error::invalid_data(
            "Extended Input Attributes body contains invalid code-plane count",
        ));
    }
    let mut cursor = off + 2;
    for _ in 0..plane_count {
        if cursor + 2 > data.len() {
            return Err(pool_walk_short());
        }
        let range_count = data[cursor + 1] as usize;
        if range_count == 0 {
            return Err(Error::invalid_data(
                "Extended Input Attributes body contains invalid range count",
            ));
        }
        cursor = cursor
            .checked_add(2)
            .and_then(|v| v.checked_add(range_count.checked_mul(4)?))
            .ok_or_else(|| Error::invalid_data("Extended Input Attributes length overflows"))?;
        if cursor > data.len() {
            return Err(pool_walk_short());
        }
    }
    Ok(cursor - off)
}

/// Total serialized body length of one object (fixed fields plus any
/// parent child/macro tail, or the type-specific leaf body), given the
/// raw pool `data` and the absolute offset `off` of the first body byte.
///
/// This is the parse-by-type walker that lets [`ObjectPool::deserialize`]
/// recover object boundaries without a per-object length prefix. Variable
/// types peek their embedded length/count field; every peek is bounds
/// checked, and the caller validates the returned extent against the
/// buffer, so malformed pools error rather than panic.
fn object_body_total_len(r#type: ObjectType, data: &[u8], off: usize) -> Result<usize> {
    use ObjectType as T;

    // Parent objects: fixed fields, then `[num_objects:u8][records][num_macros:u8][macros]`.
    if r#type == T::WorkingSet {
        let num_objects = peek_u8_at(data, off + 4)?;
        let num_macros = peek_u8_at(data, off + 5)?;
        let num_languages = peek_u8_at(data, off + 6)?;
        let child_bytes = num_objects
            .checked_mul(6)
            .ok_or_else(|| Error::invalid_data("Working Set child-list length overflows"))?;
        let macro_bytes = num_macros
            .checked_mul(2)
            .ok_or_else(|| Error::invalid_data("Working Set macro-list length overflows"))?;
        let language_bytes = num_languages
            .checked_mul(2)
            .ok_or_else(|| Error::invalid_data("Working Set language-list length overflows"))?;
        return 7usize
            .checked_add(child_bytes)
            .and_then(|n| n.checked_add(macro_bytes))
            .and_then(|n| n.checked_add(language_bytes))
            .ok_or_else(|| Error::invalid_data("Working Set body length overflows"));
    }
    if r#type == T::WindowMask {
        let fixed = window_mask_child_list_offset(data, off)?;
        let num_objects = peek_u8_at(data, off + fixed)?;
        let num_macros = peek_u8_at(data, off + fixed + 1)?;
        let child_bytes = num_objects * 6;
        return Ok(fixed + 2 + child_bytes + num_macros * 2);
    }
    if let Some((fixed, record_size)) = parent_layout(r#type) {
        // ISO 11783-6 parent tail: both counts precede both lists —
        // `[num_objects][num_macros][object records][macro records]`.
        let num_objects = peek_u8_at(data, off + fixed)?;
        let num_macros = peek_u8_at(data, off + fixed + 1)?;
        let child_bytes = num_objects * record_size;
        return Ok(fixed + 2 + child_bytes + num_macros * 2);
    }

    if r#type == T::PictureGraphic {
        // Special tail: `[fixed 13 incl data-length][num_macros][raw data][macro refs]`
        // — the macro count sits *before* the pixel data, the refs after it.
        let data_len = peek_u32_at(data, off + 9)?;
        let num_macros = peek_u8_at(data, off + 13)?;
        return Ok(14 + data_len + num_macros * 2);
    }

    // Leaf objects: the per-type body, then (for most) a trailing
    // `[number of macros][macro refs]` list, exactly like parents.
    let body = leaf_body_only_len(r#type, data, off)?;
    if leaf_has_macro_tail(r#type) {
        let num_macros = peek_u8_at(data, off + body)?;
        Ok(body + 1 + num_macros * 2)
    } else {
        Ok(body)
    }
}

/// Whether a non-parent ("leaf") object carries the standard trailing
/// `[number of macros][macro refs]` list. Mirrors which object cases call
/// `parse_object_macro_reference` in the ISO 11783-6 reference parser.
fn leaf_has_macro_tail(r#type: ObjectType) -> bool {
    use ObjectType as T;
    matches!(
        r#type,
        T::InputBoolean
            | T::InputString
            | T::InputNumber
            | T::InputList
            | T::OutputString
            | T::OutputNumber
            | T::OutputList
            | T::Line
            | T::Rectangle
            | T::Ellipse
            | T::Polygon
            | T::Meter
            | T::LinearBarGraph
            | T::ArchedBarGraph
            | T::FontAttributes
            | T::LineAttributes
            | T::FillAttributes
            | T::InputAttributes
            | T::AuxControlDesig
            | T::ScaledGraphic
    )
}

/// Length of a leaf object's body **excluding** any trailing macro list.
fn leaf_body_only_len(r#type: ObjectType, data: &[u8], off: usize) -> Result<usize> {
    use ObjectType as T;
    let len = match r#type {
        // Fixed-length leaves (length == `encode()` output).
        T::InputBoolean => 9,
        T::InputString => 14,
        T::InputNumber => 34,
        T::OutputNumber => 25,
        T::Line => 7,
        T::Rectangle => 9,
        T::Ellipse => 11,
        T::Meter => 17,
        T::LinearBarGraph => 20,
        T::ArchedBarGraph => 23,
        T::NumberVariable => 4,
        T::FontAttributes => 4,
        T::LineAttributes => 4,
        T::FillAttributes => 4,
        T::ObjectPointer => 2,
        T::AuxFunction => 4,
        T::AuxInput => 3,
        T::AuxFunction2 => 6,
        T::AuxInput2 => 7,
        T::ScaledGraphic => 8,
        T::GraphicContext => 31,
        T::ExternalReferenceName => 9,
        T::ExternalObjectPointer => 6,
        T::GraphicsContext => 12,
        T::ScaledBitmap => 16,

        // Variable-length leaves (peek the embedded length/count field).
        T::InputList => 9 + peek_u8_at(data, off + 7)? * 2,
        T::OutputList => 8 + peek_u8_at(data, off + 7)? * 2,
        T::OutputString => 13 + peek_u16_at(data, off + 11)?,
        T::Polygon => 10 + peek_u8_at(data, off + 9)? * 4,
        T::PictureGraphic => 13 + peek_u32_at(data, off + 9)?,
        T::StringVariable => 2 + peek_u16_at(data, off)?,
        T::AuxControlDesig => 3 + peek_u8_at(data, off + 2)?,
        T::GraphicData => 5 + peek_u32_at(data, off + 1)?,
        T::ColourMap => 2 + peek_u16_at(data, off)?,
        T::ExternalObjectDefinition => 10 + peek_u8_at(data, off + 9)? * 2,
        T::ColourPalette => {
            let count = peek_u16_at(data, off + 1)?;
            count
                .checked_mul(4)
                .and_then(|bytes| 3usize.checked_add(bytes))
                .ok_or_else(|| Error::invalid_data("Colour Palette body length overflows"))?
        }
        T::ObjectLabelRef => 2 + peek_u16_at(data, off)? * 7,
        T::InputAttributes => 2 + peek_u8_at(data, off + 1)?,
        T::ExtendedInputAttributes => extended_input_attributes_len(data, off)?,
        T::WorkingSetSpecialControls => 2 + peek_u16_at(data, off)?,
        T::Macro => 2 + peek_u16_at(data, off)?,

        // Parent types are handled above by `parent_layout`.
        T::WorkingSet
        | T::DataMask
        | T::AlarmMask
        | T::Container
        | T::SoftKeyMask
        | T::Key
        | T::Button
        | T::WindowMask
        | T::KeyGroup
        | T::Animation => unreachable!("parent types handled by parent_layout"),
    };
    Ok(len)
}

#[inline]
#[must_use]
const fn parent_record_size(r#type: ObjectType) -> Option<usize> {
    match r#type {
        ObjectType::WindowMask => Some(6),
        _ => match parent_layout(r#type) {
            Some((_, record_size)) => Some(record_size),
            None => None,
        },
    }
}

#[must_use]
pub fn scaled_graphic_value_source_is_valid(pool: &ObjectPool, value: ObjectID) -> bool {
    scaled_graphic_value_source_is_valid_inner(pool, value, &mut Vec::new())
}

fn scaled_graphic_value_source_is_valid_inner(
    pool: &ObjectPool,
    value: ObjectID,
    visited_pointers: &mut Vec<ObjectID>,
) -> bool {
    if value == ObjectID::NULL {
        return true;
    }
    let Some(object) = pool.find(value) else {
        return false;
    };
    match object.r#type {
        ObjectType::GraphicData | ObjectType::PictureGraphic => true,
        ObjectType::ObjectPointer => {
            if visited_pointers.contains(&value) {
                return false;
            }
            let Ok(body) = object.get_object_pointer_body() else {
                return false;
            };
            visited_pointers.push(value);
            let valid =
                scaled_graphic_value_source_is_valid_inner(pool, body.value, visited_pointers);
            visited_pointers.pop();
            valid
        }
        _ => false,
    }
}

fn scaled_graphic_value_source_uses_pointer(
    pool: &ObjectPool,
    value: ObjectID,
    pointer: ObjectID,
    visited_pointers: &mut Vec<ObjectID>,
) -> bool {
    if value == pointer {
        return true;
    }
    if value == ObjectID::NULL {
        return false;
    }
    let Some(object) = pool.find(value) else {
        return false;
    };
    if object.r#type != ObjectType::ObjectPointer || visited_pointers.contains(&value) {
        return false;
    }
    let Ok(body) = object.get_object_pointer_body() else {
        return false;
    };
    visited_pointers.push(value);
    let uses =
        scaled_graphic_value_source_uses_pointer(pool, body.value, pointer, visited_pointers);
    visited_pointers.pop();
    uses
}

fn scaled_graphic_pointer_retarget_is_valid(
    pool: &ObjectPool,
    pointer: ObjectID,
    value: ObjectID,
) -> bool {
    if value == pointer {
        return false;
    }
    let mut visited = vec![pointer];
    scaled_graphic_value_source_is_valid_inner(pool, value, &mut visited)
}

/// Validate a Change Numeric Value retarget for an Object Pointer in the
/// context where that pointer is used.
///
/// A standalone Object Pointer can point at NULL or any uploaded object.
/// When the same Object Pointer is a Soft Key Mask child, its value is a
/// soft-key slot and must stay NULL or point at a Key. When it is a Key Group
/// child, the slot must stay an actual Key target and cannot be NULL. When a
/// Scaled Graphic reaches the pointer through its Value chain, the pointer must
/// continue to resolve only to NULL, Graphic Data, Picture Graphic, or another
/// valid Object Pointer chain. This mirrors object-pool upload validation for
/// later runtime retargets.
#[must_use]
pub fn object_pointer_numeric_value_is_valid_for_context(
    pool: &ObjectPool,
    pointer: ObjectID,
    value: ObjectID,
) -> bool {
    if value != ObjectID::NULL && pool.find(value).is_none() {
        return false;
    }

    for owner in pool.objects().iter().filter(|owner| {
        owner.children.contains(&pointer)
            || owner.children_pos.iter().any(|child| child.id == pointer)
    }) {
        match owner.r#type {
            ObjectType::SoftKeyMask => {
                if value != ObjectID::NULL
                    && !pool
                        .find(value)
                        .is_some_and(|object| object.r#type == ObjectType::Key)
                {
                    return false;
                }
            }
            ObjectType::KeyGroup => {
                if value == ObjectID::NULL
                    || !pool
                        .find(value)
                        .is_some_and(|object| object.r#type == ObjectType::Key)
                {
                    return false;
                }
            }
            _ => {}
        }
    }

    for owner in pool
        .objects()
        .iter()
        .filter(|owner| owner.r#type == ObjectType::OutputList)
    {
        let Ok(body) = owner.get_output_list_body() else {
            return false;
        };
        if body.items.contains(&pointer) && !output_list_item_reference_is_valid(pool, value) {
            return false;
        }
    }

    for owner in pool
        .objects()
        .iter()
        .filter(|owner| owner.r#type == ObjectType::ScaledGraphic)
    {
        let Ok(body) = owner.get_scaled_graphic_body() else {
            return false;
        };
        if scaled_graphic_value_source_uses_pointer(pool, body.value, pointer, &mut Vec::new())
            && !scaled_graphic_pointer_retarget_is_valid(pool, pointer, value)
        {
            return false;
        }
    }

    true
}

/// Validate an External Object Pointer default-object retarget in the context
/// where that pointer is used.
///
/// ISO Soft Key Masks may contain External Object Pointer slots. Their local
/// default object is the fallback soft-key slot and must remain NULL or a Key
/// after Change Attribute replay, just like it was required to be during pool
/// upload validation.
#[must_use]
pub fn external_object_pointer_default_is_valid_for_context(
    pool: &ObjectPool,
    pointer: ObjectID,
    value: ObjectID,
) -> bool {
    if value != ObjectID::NULL && pool.find(value).is_none() {
        return false;
    }

    for owner in pool.objects().iter().filter(|owner| {
        owner.children.contains(&pointer)
            || owner.children_pos.iter().any(|child| child.id == pointer)
    }) {
        if matches!(owner.r#type, ObjectType::SoftKeyMask | ObjectType::KeyGroup)
            && value != ObjectID::NULL
            && !pool
                .find(value)
                .is_some_and(|object| object.r#type == ObjectType::Key)
        {
            return false;
        }
    }

    for owner in pool
        .objects()
        .iter()
        .filter(|owner| owner.r#type == ObjectType::OutputList)
    {
        let Ok(body) = owner.get_output_list_body() else {
            return false;
        };
        if body.items.contains(&pointer) && !output_list_item_reference_is_valid(pool, value) {
            return false;
        }
    }

    true
}

