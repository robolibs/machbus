fn png_unfilter_row(
    filter: u8,
    bytes_per_pixel: usize,
    previous: &[u8],
    row: &mut [u8],
) -> Result<(), &'static str> {
    match filter {
        0 => {}
        1 => {
            for i in 0..row.len() {
                let left = i
                    .checked_sub(bytes_per_pixel)
                    .and_then(|index| row.get(index))
                    .copied()
                    .unwrap_or(0);
                row[i] = row[i].wrapping_add(left);
            }
        }
        2 => {
            for (pixel, up) in row.iter_mut().zip(previous.iter().copied()) {
                *pixel = pixel.wrapping_add(up);
            }
        }
        3 => {
            for i in 0..row.len() {
                let left = i
                    .checked_sub(bytes_per_pixel)
                    .and_then(|index| row.get(index))
                    .copied()
                    .unwrap_or(0);
                let up = previous.get(i).copied().unwrap_or(0);
                row[i] = row[i].wrapping_add(((u16::from(left) + u16::from(up)) / 2) as u8);
            }
        }
        4 => {
            for i in 0..row.len() {
                let left = i
                    .checked_sub(bytes_per_pixel)
                    .and_then(|index| row.get(index))
                    .copied()
                    .unwrap_or(0);
                let up = previous.get(i).copied().unwrap_or(0);
                let up_left = i
                    .checked_sub(bytes_per_pixel)
                    .and_then(|index| previous.get(index))
                    .copied()
                    .unwrap_or(0);
                row[i] = row[i].wrapping_add(png_paeth(left, up, up_left));
            }
        }
        _ => return Err("PNG scanline uses an invalid filter type"),
    }
    Ok(())
}

fn png_paeth(left: u8, up: u8, up_left: u8) -> u8 {
    let left = i32::from(left);
    let up = i32::from(up);
    let up_left = i32::from(up_left);
    let p = left + up - up_left;
    let pa = (p - left).abs();
    let pb = (p - up).abs();
    let pc = (p - up_left).abs();
    if pa <= pb && pa <= pc {
        left as u8
    } else if pb <= pc {
        up as u8
    } else {
        up_left as u8
    }
}

fn zlib_deflate_decompress(data: &[u8]) -> Result<Vec<u8>, &'static str> {
    if data.len() < 6 {
        return Err("PNG IDAT zlib stream is too short");
    }
    let cmf = data[0];
    let flg = data[1];
    if cmf & 0x0F != 8 {
        return Err("PNG zlib stream does not use DEFLATE compression");
    }
    if (u16::from(cmf) * 256 + u16::from(flg)) % 31 != 0 {
        return Err("PNG zlib header check bits are invalid");
    }
    if flg & 0x20 != 0 {
        return Err("PNG zlib preset dictionaries are unsupported");
    }
    let expected_adler = u32::from_be_bytes([
        data[data.len() - 4],
        data[data.len() - 3],
        data[data.len() - 2],
        data[data.len() - 1],
    ]);
    let deflate = &data[2..data.len() - 4];
    let mut reader = DeflateBitReader::new(deflate);
    let mut out = Vec::new();
    loop {
        let final_block = reader.read_bit()? != 0;
        let block_type = reader.read_bits_lsb(2)?;
        match block_type {
            0 => decode_stored_deflate_block(&mut reader, &mut out)?,
            1 => decode_fixed_huffman_deflate_block(&mut reader, &mut out)?,
            2 => decode_dynamic_huffman_deflate_block(&mut reader, &mut out)?,
            _ => return Err("PNG zlib stream uses reserved DEFLATE block type"),
        }
        if final_block {
            if !reader.remaining_bits_are_zero() {
                return Err("PNG DEFLATE stream has trailing bytes after final block");
            }
            break;
        }
    }
    if adler32(&out) != expected_adler {
        return Err("PNG zlib Adler-32 check failed");
    }
    Ok(out)
}

struct DeflateBitReader<'a> {
    data: &'a [u8],
    bit_offset: usize,
}

impl<'a> DeflateBitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_offset: 0,
        }
    }

    fn read_bit(&mut self) -> Result<u8, &'static str> {
        let byte = self
            .data
            .get(self.bit_offset / 8)
            .ok_or("PNG DEFLATE stream ended mid-bit")?;
        let bit = (byte >> (self.bit_offset % 8)) & 1;
        self.bit_offset += 1;
        Ok(bit)
    }

    fn read_bits_lsb(&mut self, count: u8) -> Result<u16, &'static str> {
        let mut value = 0u16;
        for bit in 0..count {
            value |= u16::from(self.read_bit()?) << bit;
        }
        Ok(value)
    }

    fn align_to_byte(&mut self) {
        self.bit_offset = self.bit_offset.div_ceil(8) * 8;
    }

    fn read_aligned_byte(&mut self) -> Result<u8, &'static str> {
        if !self.bit_offset.is_multiple_of(8) {
            return Err("PNG DEFLATE stored block is not byte aligned");
        }
        let byte = self
            .data
            .get(self.bit_offset / 8)
            .copied()
            .ok_or("PNG DEFLATE stream ended mid-byte")?;
        self.bit_offset += 8;
        Ok(byte)
    }

    fn remaining_bits_are_zero(&self) -> bool {
        let mut offset = self.bit_offset;
        while offset < self.data.len() * 8 {
            let byte = self.data[offset / 8];
            if (byte >> (offset % 8)) & 1 != 0 {
                return false;
            }
            offset += 1;
        }
        true
    }
}

fn decode_stored_deflate_block(
    reader: &mut DeflateBitReader<'_>,
    out: &mut Vec<u8>,
) -> Result<(), &'static str> {
    reader.align_to_byte();
    let len_lo = reader.read_aligned_byte()?;
    let len_hi = reader.read_aligned_byte()?;
    let nlen_lo = reader.read_aligned_byte()?;
    let nlen_hi = reader.read_aligned_byte()?;
    let len = u16::from_le_bytes([len_lo, len_hi]);
    let nlen = u16::from_le_bytes([nlen_lo, nlen_hi]);
    if len != !nlen {
        return Err("PNG stored DEFLATE block length check failed");
    }
    for _ in 0..len {
        out.push(reader.read_aligned_byte()?);
    }
    Ok(())
}

fn decode_fixed_huffman_deflate_block(
    reader: &mut DeflateBitReader<'_>,
    out: &mut Vec<u8>,
) -> Result<(), &'static str> {
    loop {
        let symbol = decode_fixed_literal_length_symbol(reader)?;
        match symbol {
            0..=255 => out.push(u8::try_from(symbol).unwrap_or_default()),
            256 => break,
            257..=285 => {
                let (length_base, length_extra) = deflate_length_base_extra(symbol)?;
                let length =
                    usize::from(length_base) + usize::from(reader.read_bits_lsb(length_extra)?);
                let distance_symbol = decode_fixed_distance_symbol(reader)?;
                let (distance_base, distance_extra) = deflate_distance_base_extra(distance_symbol)?;
                let distance =
                    usize::from(distance_base) + usize::from(reader.read_bits_lsb(distance_extra)?);
                if distance == 0 || distance > out.len() {
                    return Err("PNG fixed DEFLATE distance exceeds output size");
                }
                for _ in 0..length {
                    let index = out.len() - distance;
                    out.push(out[index]);
                }
            }
            _ => return Err("PNG fixed DEFLATE literal/length symbol is reserved"),
        }
    }
    Ok(())
}

fn decode_dynamic_huffman_deflate_block(
    reader: &mut DeflateBitReader<'_>,
    out: &mut Vec<u8>,
) -> Result<(), &'static str> {
    const CODE_LENGTH_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];
    let literal_length_count = usize::from(reader.read_bits_lsb(5)?) + 257;
    let distance_count = usize::from(reader.read_bits_lsb(5)?) + 1;
    let code_length_count = usize::from(reader.read_bits_lsb(4)?) + 4;
    if literal_length_count > 286 || distance_count > 32 || code_length_count > 19 {
        return Err("PNG dynamic DEFLATE header counts are invalid");
    }

    let mut code_length_lengths = vec![0u8; 19];
    for &symbol in CODE_LENGTH_ORDER.iter().take(code_length_count) {
        code_length_lengths[symbol] = u8::try_from(reader.read_bits_lsb(3)?).unwrap_or_default();
    }
    let code_length_table = HuffmanTable::from_lengths(&code_length_lengths)?;
    let total_lengths = literal_length_count
        .checked_add(distance_count)
        .ok_or("PNG dynamic DEFLATE code count overflows")?;
    let mut lengths = Vec::with_capacity(total_lengths);
    while lengths.len() < total_lengths {
        let symbol = code_length_table.decode(reader)?;
        match symbol {
            0..=15 => lengths.push(u8::try_from(symbol).unwrap_or_default()),
            16 => {
                let Some(&previous) = lengths.last() else {
                    return Err("PNG dynamic DEFLATE repeats a missing code length");
                };
                let repeat = usize::from(reader.read_bits_lsb(2)?) + 3;
                if lengths.len() + repeat > total_lengths {
                    return Err("PNG dynamic DEFLATE code-length repeat overruns table");
                }
                lengths.resize(lengths.len() + repeat, previous);
            }
            17 => {
                let repeat = usize::from(reader.read_bits_lsb(3)?) + 3;
                if lengths.len() + repeat > total_lengths {
                    return Err("PNG dynamic DEFLATE zero repeat overruns table");
                }
                lengths.resize(lengths.len() + repeat, 0);
            }
            18 => {
                let repeat = usize::from(reader.read_bits_lsb(7)?) + 11;
                if lengths.len() + repeat > total_lengths {
                    return Err("PNG dynamic DEFLATE long zero repeat overruns table");
                }
                lengths.resize(lengths.len() + repeat, 0);
            }
            _ => return Err("PNG dynamic DEFLATE code-length symbol is invalid"),
        }
    }

    let literal_lengths = &lengths[..literal_length_count];
    if literal_lengths.get(256).copied().unwrap_or(0) == 0 {
        return Err("PNG dynamic DEFLATE has no end-of-block code");
    }
    let distance_lengths = &lengths[literal_length_count..];
    let literal_table = HuffmanTable::from_lengths(literal_lengths)?;
    let distance_table = HuffmanTable::from_lengths_allow_empty(distance_lengths)?;

    loop {
        let symbol = literal_table.decode(reader)?;
        match symbol {
            0..=255 => out.push(u8::try_from(symbol).unwrap_or_default()),
            256 => break,
            257..=285 => {
                let (length_base, length_extra) = deflate_length_base_extra(symbol)?;
                let length =
                    usize::from(length_base) + usize::from(reader.read_bits_lsb(length_extra)?);
                let distance_symbol = distance_table.decode(reader)?;
                let distance_symbol = u8::try_from(distance_symbol)
                    .map_err(|_| "PNG dynamic DEFLATE distance symbol is invalid")?;
                let (distance_base, distance_extra) = deflate_distance_base_extra(distance_symbol)?;
                let distance =
                    usize::from(distance_base) + usize::from(reader.read_bits_lsb(distance_extra)?);
                if distance == 0 || distance > out.len() {
                    return Err("PNG dynamic DEFLATE distance exceeds output size");
                }
                for _ in 0..length {
                    let index = out.len() - distance;
                    out.push(out[index]);
                }
            }
            _ => return Err("PNG dynamic DEFLATE literal/length symbol is reserved"),
        }
    }
    Ok(())
}

struct HuffmanEntry {
    code: u16,
    len: u8,
    symbol: u16,
}

struct HuffmanTable {
    entries: Vec<HuffmanEntry>,
    max_bits: u8,
}

impl HuffmanTable {
    fn from_lengths(lengths: &[u8]) -> Result<Self, &'static str> {
        let table = Self::from_lengths_allow_empty(lengths)?;
        if table.entries.is_empty() {
            return Err("PNG DEFLATE Huffman table is empty");
        }
        Ok(table)
    }

    fn from_lengths_allow_empty(lengths: &[u8]) -> Result<Self, &'static str> {
        let mut counts = [0u16; 16];
        let mut max_bits = 0u8;
        for &len in lengths {
            if len > 15 {
                return Err("PNG DEFLATE Huffman code length exceeds 15 bits");
            }
            if len != 0 {
                counts[usize::from(len)] += 1;
                max_bits = max_bits.max(len);
            }
        }
        let mut code = 0u16;
        let mut next_code = [0u16; 16];
        for bits in 1..=15 {
            code = (code + counts[bits - 1]) << 1;
            if code + counts[bits] > (1u16 << bits) {
                return Err("PNG DEFLATE Huffman table is over-subscribed");
            }
            next_code[bits] = code;
        }
        let mut entries = Vec::new();
        for (symbol, &len) in lengths.iter().enumerate() {
            if len == 0 {
                continue;
            }
            let code = next_code[usize::from(len)];
            next_code[usize::from(len)] += 1;
            entries.push(HuffmanEntry {
                code: reverse_low_bits(code, len),
                len,
                symbol: u16::try_from(symbol)
                    .map_err(|_| "PNG DEFLATE Huffman symbol exceeds u16")?,
            });
        }
        Ok(Self { entries, max_bits })
    }

    fn decode(&self, reader: &mut DeflateBitReader<'_>) -> Result<u16, &'static str> {
        if self.entries.is_empty() {
            return Err("PNG DEFLATE Huffman table is empty");
        }
        let mut code = 0u16;
        for bits in 1..=self.max_bits {
            code |= u16::from(reader.read_bit()?) << (bits - 1);
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| entry.len == bits && entry.code == code)
            {
                return Ok(entry.symbol);
            }
        }
        Err("PNG DEFLATE Huffman code is invalid")
    }
}

fn decode_fixed_literal_length_symbol(
    reader: &mut DeflateBitReader<'_>,
) -> Result<u16, &'static str> {
    let mut code = 0u16;
    for bits in 1..=9 {
        code |= u16::from(reader.read_bit()?) << (bits - 1);
        let canonical = reverse_low_bits(code, bits);
        match bits {
            7 if canonical <= 0b0010111 => return Ok(256 + canonical),
            8 if (0b0011_0000..=0b1011_1111).contains(&canonical) => {
                return Ok(canonical - 0b0011_0000);
            }
            8 if (0b1100_0000..=0b1100_0111).contains(&canonical) => {
                return Ok(280 + (canonical - 0b1100_0000));
            }
            9 if (0b1_1001_0000..=0b1_1111_1111).contains(&canonical) => {
                return Ok(144 + (canonical - 0b1_1001_0000));
            }
            _ => {}
        }
    }
    Err("PNG fixed DEFLATE literal/length code is invalid")
}

fn decode_fixed_distance_symbol(reader: &mut DeflateBitReader<'_>) -> Result<u8, &'static str> {
    let code = reader.read_bits_lsb(5)?;
    Ok(u8::try_from(reverse_low_bits(code, 5)).unwrap_or_default())
}

fn reverse_low_bits(mut value: u16, bits: u8) -> u16 {
    let mut reversed = 0u16;
    for _ in 0..bits {
        reversed = (reversed << 1) | (value & 1);
        value >>= 1;
    }
    reversed
}

fn deflate_length_base_extra(symbol: u16) -> Result<(u16, u8), &'static str> {
    const LENGTH_BASES: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LENGTH_EXTRAS: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    let index = usize::from(
        symbol
            .checked_sub(257)
            .ok_or("PNG fixed DEFLATE length symbol is invalid")?,
    );
    let Some((&base, &extra)) = LENGTH_BASES.get(index).zip(LENGTH_EXTRAS.get(index)) else {
        return Err("PNG fixed DEFLATE length symbol is invalid");
    };
    Ok((base, extra))
}

fn deflate_distance_base_extra(symbol: u8) -> Result<(u16, u8), &'static str> {
    const DISTANCE_BASES: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DISTANCE_EXTRAS: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    let index = usize::from(symbol);
    let Some((&base, &extra)) = DISTANCE_BASES.get(index).zip(DISTANCE_EXTRAS.get(index)) else {
        return Err("PNG fixed DEFLATE distance symbol is reserved");
    };
    Ok((base, extra))
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + u32::from(byte)) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtraBitmapData {
    Reject,
    Ignore,
}

#[expect(
    clippy::too_many_arguments,
    reason = "keeps graphics placeholder reasons consistent"
)]
fn push_indexed_image_or_placeholder(
    out: &mut Vec<RenderCommand>,
    object_id: ObjectID,
    rect: Rect,
    object_type: ObjectType,
    label: &'static str,
    width: u16,
    height: u16,
    format: u8,
    transparent: bool,
    transparency: u8,
    mut data: Vec<u8>,
    extra_data: ExtraBitmapData,
) {
    let Some(required) = indexed_bitmap_required_bytes(width, height, format) else {
        out.push(RenderCommand::Placeholder {
            rect,
            object_type,
            reason: format!("{label} {width}x{height} fmt={format} has unsupported bitmap format"),
        });
        return;
    };
    if data.len() < required {
        out.push(RenderCommand::Placeholder {
            rect,
            object_type,
            reason: format!(
                "{label} {width}x{height} fmt={format} has short bitmap payload: {} < {required}",
                data.len()
            ),
        });
        return;
    }
    if data.len() > required {
        match extra_data {
            ExtraBitmapData::Reject => {
                out.push(RenderCommand::Placeholder {
                    rect,
                    object_type,
                    reason: format!(
                        "{label} {width}x{height} fmt={format} has long bitmap payload: {} > {required}",
                        data.len()
                    ),
                });
                return;
            }
            ExtraBitmapData::Ignore => {
                data.truncate(required);
            }
        }
    }
    out.push(RenderCommand::IndexedImage {
        object_id,
        rect,
        width,
        height,
        format,
        transparent,
        transparency,
        data,
    });
}

/// Expand a tightly packed 24-bit RGB bitmap (ScaledBitmap format 3) into an
/// opaque RGBA8 image command, or emit a precise placeholder when the payload
/// length does not match `width * height * 3`.
#[expect(
    clippy::too_many_arguments,
    reason = "mirrors push_indexed_image_or_placeholder for consistent placeholder reasons"
)]
fn push_rgb24_image_or_placeholder(
    out: &mut Vec<RenderCommand>,
    object_id: ObjectID,
    rect: Rect,
    object_type: ObjectType,
    label: &'static str,
    width: u16,
    height: u16,
    data: Vec<u8>,
) {
    let Some(required) = usize::from(width)
        .checked_mul(usize::from(height))
        .and_then(|pixels| pixels.checked_mul(3))
    else {
        out.push(RenderCommand::Placeholder {
            rect,
            object_type,
            reason: format!("{label} {width}x{height} fmt=3 pixel count overflows"),
        });
        return;
    };
    if data.len() != required {
        out.push(RenderCommand::Placeholder {
            rect,
            object_type,
            reason: format!(
                "{label} {width}x{height} fmt=3 has mismatched 24-bit RGB payload: {} != {required}",
                data.len()
            ),
        });
        return;
    }
    let mut rgba = Vec::with_capacity(required + required / 3);
    for pixel in data.chunks_exact(3) {
        rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 0xFF]);
    }
    out.push(RenderCommand::RgbaImage {
        object_id,
        rect,
        width,
        height,
        data: rgba,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::ObjectID;
    use crate::isobus::vt::render::scene::Scene;

    fn empty_scene() -> Scene {
        Scene::new(ObjectID::new(1), (10, 10))
    }

    #[test]
    fn indexed_bitmap_required_bytes_counts_padding_per_row() {
        assert_eq!(indexed_bitmap_required_bytes(10, 2, 0), Some(4));
        assert_eq!(indexed_bitmap_required_bytes(3, 2, 1), Some(4));
        assert_eq!(indexed_bitmap_required_bytes(3, 2, 2), Some(6));
        assert_eq!(indexed_bitmap_required_bytes(0, 2, 0), Some(0));
        assert_eq!(indexed_bitmap_required_bytes(3, 2, 9), None);
    }

    #[test]
    fn render_empty_scene_emits_background_and_clip() {
        let r = GtuiRenderer::default();
        let cmds = r.render(&empty_scene());
        // At least: background fill + clip.
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCommand::FillRect { .. }))
        );
        assert!(cmds.iter().any(|c| matches!(c, RenderCommand::Clip(_))));
    }

    #[test]
    fn output_string_emits_background_and_text() {
        let mut scene = empty_scene();
        scene.nodes.push(SceneNode {
            id: ObjectID::new(2),
            object_type: ObjectType::OutputString,
            parent: ObjectID::new(1),
            rect: Rect::new(5, 5, 50, 20),
            clip: None,
            style: solid_style(Colour::rgb(0, 0, 0), Colour::rgb(255, 255, 255)),
            visible: true,
            enabled: true,
            kind: NodeKind::OutputString {
                text: "HI".into(),
                transparent_bg: false,
                justification: 1,
            },
        });
        let r = GtuiRenderer::default();
        let cmds = r.render(&scene);
        let has_text = cmds.iter().any(|c| {
            matches!(
                c,
                RenderCommand::DrawText { text, .. } if text == "HI"
            )
        });
        assert!(has_text);
    }

    #[test]
    fn draw_text_command_carries_renderer_ready_layout() {
        let mut scene = empty_scene();
        let mut style = solid_style(Colour::rgb(0, 0, 0), Colour::rgb(255, 255, 255));
        style.font = crate::isobus::vt::render::style::FontMetrics {
            cell_w: 6,
            cell_h: 12,
            ascent: 10,
            descent: 2,
        };
        scene.nodes.push(SceneNode {
            id: ObjectID::new(20),
            object_type: ObjectType::OutputString,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 12, 24),
            clip: None,
            style,
            visible: true,
            enabled: true,
            kind: NodeKind::OutputString {
                text: "ABCDE".into(),
                transparent_bg: true,
                justification: 2,
            },
        });

        let cmds = GtuiRenderer::default().render(&scene);
        let layout = cmds.iter().find_map(|cmd| match cmd {
            RenderCommand::DrawText { layout, .. } => Some(layout),
            _ => None,
        });

        let layout = layout.expect("output string emits text layout");
        assert_eq!(layout.rendered(), "AB\nCD");
        assert_eq!(layout.clipped_rows, 1);
        assert_eq!(layout.align, HorizontalAlign::Right);
    }

    #[test]
    fn output_rectangle_emits_fill_and_stroke() {
        let mut scene = empty_scene();
        let mut style = solid_style(Colour::rgb(10, 10, 10), Colour::rgb(0, 0, 0));
        style.fill_type = FillType::FillColour;
        scene.nodes.push(SceneNode {
            id: ObjectID::new(3),
            object_type: ObjectType::Rectangle,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 40, 40),
            clip: None,
            style,
            visible: true,
            enabled: true,
            kind: NodeKind::OutputRectangle {
                line_suppression: 0,
                fill_pattern: None,
            },
        });
        let r = GtuiRenderer::default();
        let cmds = r.render(&scene);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCommand::FillRect { .. }))
        );
        assert!(
            cmds.iter()
                .any(|c| matches!(c, RenderCommand::StrokeRect { .. }))
        );
    }

    #[test]
    fn disabled_input_is_dimmed() {
        let mut scene = empty_scene();
        scene.nodes.push(SceneNode {
            id: ObjectID::new(4),
            object_type: ObjectType::InputBoolean,
            parent: ObjectID::new(1),
            rect: Rect::new(0, 0, 30, 30),
            clip: None,
            style: solid_style(Colour::rgb(0, 0, 0), Colour::rgb(255, 255, 255)),
            visible: true,
            enabled: false,
            kind: NodeKind::InputBoolean {
                enabled: false,
                value: false,
            },
        });
        let r = GtuiRenderer::default();
        let cmds = r.render(&scene);
        // A disabled field emits at least two stroke rects (outer + dim).
        let stroke_count = cmds
            .iter()
            .filter(|c| matches!(c, RenderCommand::StrokeRect { .. }))
            .count();
        assert!(stroke_count >= 2);
    }

    #[test]
    fn unsupported_marker_is_emitted() {
        let mut scene = empty_scene();
        scene.unsupported.push(UnsupportedRecord {
            id: ObjectID::new(9),
            object_type: ObjectType::Animation,
            reason: "no animation runtime",
        });
        let r = GtuiRenderer::default();
        let cmds = r.render(&scene);
        assert!(cmds.iter().any(|c| matches!(
            c,
            RenderCommand::Placeholder {
                object_type: ObjectType::Animation,
                ..
            }
        )));
    }

    #[test]
    fn count_drawables_skips_unsupported_and_invisible() {
        let mut scene = empty_scene();
        scene.nodes.push(SceneNode {
            id: ObjectID::new(1),
            object_type: ObjectType::OutputString,
            parent: ObjectID::NULL,
            rect: Rect::default(),
            clip: None,
            style: ResolvedStyle::default(),
            visible: false,
            enabled: true,
            kind: NodeKind::OutputString {
                text: "x".into(),
                transparent_bg: false,
                justification: 0,
            },
        });
        scene.nodes.push(SceneNode {
            id: ObjectID::new(2),
            object_type: ObjectType::WorkingSet,
            parent: ObjectID::NULL,
            rect: Rect::default(),
            clip: None,
            style: ResolvedStyle::default(),
            visible: true,
            enabled: true,
            kind: NodeKind::Unsupported {
                type_byte: 0,
                reason: "ws",
            },
        });
        let r = GtuiRenderer::default();
        assert_eq!(r.count_drawables(&scene), 0);
    }

    #[test]
    fn text_decode_lossy_via_helper() {
        // Sanity: the text helper round-trips ASCII.
        use crate::isobus::vt::render::text;
        assert_eq!(text::decode_lossy(b"GTUI"), "GTUI");
    }
}
