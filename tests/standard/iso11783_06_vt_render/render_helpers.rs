use machbus::isobus::vt::render::coverage::{RenderStatus, coverage_ledger, render_status_for};
use machbus::isobus::vt::render::framebuffer::FramebufferRenderer;
use machbus::isobus::vt::render::gtui::{GraphicsContextCopySource, GtuiRenderer, RenderCommand};
use machbus::isobus::vt::render::input::{
    EditState, InputRuntime, OperatorEvent, VtBusMessageKind, VtEvent,
};
use machbus::isobus::vt::render::layout::{LayoutConfig, LayoutEngine, PlacementMap};
use machbus::isobus::vt::render::scene::{NodeKind, Rect, SoftKeyKind};
use machbus::isobus::vt::render::style::{Colour, FillType, FontMetrics, Palette, ResolvedStyle};
use machbus::isobus::vt::render::text::{self, HorizontalAlign, VerticalAlign};
use machbus::isobus::vt::render::{
    ActivationHoldTiming, IopDocument, LayoutConfig as DocConfig, RenderUpdate, SceneLanguage,
    UserLayoutPlacement, VtRenderRuntime, VtRuntimeCommand,
};
use machbus::isobus::vt::{
    ActivationCode, AlarmMaskBody, AnimationBody, ArchedBarGraphBody, ButtonBody, ChildRef,
    ColourMapBody, ColourPaletteBody, ContainerBody, DataMaskBody, ExtendedInputAttributesBody,
    ExtendedInputCodePlane, ExternalObjectDefinitionBody, ExternalObjectPointerBody,
    ExternalReferenceNameBody, FillAttributesBody, FontAttributesBody, GraphicContextBody,
    GraphicDataBody, GraphicsContextBody, GraphicsContextV6, InputAttributesBody, InputBooleanBody,
    InputListBody, InputNumberBody,
    InputStringBody, KeyBody, KeyGroupBody, LanguageCountryPair, LineAttributesBody,
    LinearBarGraphBody, MacroBody, MacroCommand, MeterBody, NumberVariableBody, ObjectID,
    ObjectLabelRefBody, ObjectLabelRefEntry, ObjectLabelState, ObjectPointerBody, ObjectPool,
    ObjectType, OutputEllipseBody, OutputLineBody, OutputListBody, OutputNumberBody,
    OutputPolygonBody, OutputRectangleBody, OutputStringBody, PictureGraphicBody, PolygonPoint,
    ScaledBitmapBody, ScaledGraphicBody, ServerRenderEffect, ServerWorkingSet, SoftKeyMaskBody,
    StringVariableBody, VTObject, VTServer, VTServerConfig, WideCharRange, WindowMaskBody,
    WorkingSetBody, WorkingSetSpecialControlsBody, cmd, create_alarm_mask, create_animation,
    create_arched_bar_graph, create_button, create_colour_map, create_colour_palette,
    create_container, create_data_mask, create_extended_input_attributes,
    create_external_object_definition, create_external_object_pointer,
    create_external_reference_name, create_fill_attributes, create_font_attributes,
    create_graphic_context, create_graphic_data, create_graphics_context, create_input_attributes,
    create_input_boolean,
    create_input_list, create_input_number, create_input_string, create_key, create_key_group,
    create_line_attributes, create_linear_bar_graph, create_macro, create_meter,
    create_number_variable, create_object_label_ref, create_object_pointer, create_output_ellipse,
    create_output_line, create_output_list, create_output_number, create_output_polygon,
    create_output_rectangle, create_output_string, create_picture_graphic, create_scaled_bitmap,
    create_scaled_graphic, create_soft_key_mask, create_string_variable, create_window_mask,
    create_working_set, create_working_set_special_controls,
};
use machbus::net::Message;
use machbus::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use machbus::net::pgn_defs::{PGN_ECU_TO_VT, PGN_VT_TO_ECU};

const VT_RENDER_TRACE_HEX: &str = include_str!("../../fixtures/isobus/vt_render_trace.hex");
const VT_OBJECT_POOL_HEX: &str = include_str!("../../fixtures/isobus/vt_object_pool.hex");
const VT3_TEST_POOL_IOP: &[u8] = include_bytes!("../../fixtures/isobus/VT3TestPool.iop");
const VT_EXTERNAL_EVIDENCE_REQUIREMENTS: &str =
    include_str!("../../fixtures/isobus/vt_external_evidence_requirements.txt");
const VT_EXTERNAL_REPORTS_README: &str =
    include_str!("../../fixtures/isobus/vt_external_reports/README.md");
const PROTOCOL_MATRIX_CSV: &str =
    include_str!("../../../book/src/reference/assets/protocol_matrix.csv");
const PROJECT_MAKEFILE: &str = include_str!("../../../Makefile");

// ─── Helpers ───────────────────────────────────────────────────────

fn render(pool: &ObjectPool, active_mask: ObjectID) -> machbus::isobus::vt::render::Scene {
    LayoutEngine::new(LayoutConfig::default()).build(pool, active_mask)
}

fn minimal_png_rgba(width: u32, height: u32) -> Vec<u8> {
    minimal_png_rgba_with_idat(width, height, zlib_stored_block)
}

fn minimal_png_rgba_fixed_deflate(width: u32, height: u32) -> Vec<u8> {
    minimal_png_rgba_with_idat(width, height, zlib_fixed_literal_block)
}

fn minimal_png_rgba_dynamic_deflate(width: u32, height: u32) -> Vec<u8> {
    minimal_png_rgba_with_idat(width, height, zlib_dynamic_literal_block)
}

fn minimal_png_rgba_dynamic_deflate_backref(width: u32, height: u32) -> Vec<u8> {
    minimal_png_rgba_with_idat(width, height, zlib_dynamic_backref_block)
}

fn minimal_png_rgba_filter_suite() -> Vec<u8> {
    const WIDTH: usize = 3;
    const FILTERS: [u8; 5] = [0, 1, 2, 3, 4];
    let mut raw = Vec::new();
    let mut previous = vec![0u8; WIDTH * 4];
    for (row_index, &filter) in FILTERS.iter().enumerate() {
        let decoded = png_filter_suite_row(row_index);
        raw.push(filter);
        raw.extend_from_slice(&encode_png_filtered_row(filter, &decoded, &previous, 4));
        previous = decoded;
    }
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 3, 0, 0, 0, 5, 8, 6, 0, 0, 0]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn expected_png_filter_suite_rgba() -> Vec<u8> {
    let mut out = Vec::new();
    for row in 0..5 {
        out.extend_from_slice(&png_filter_suite_row(row));
    }
    out
}

fn png_filter_suite_row(row: usize) -> Vec<u8> {
    let mut out = Vec::new();
    for x in 0..3usize {
        out.extend_from_slice(&[
            u8::try_from(17 + row * 29 + x * 11).unwrap(),
            u8::try_from(31 + row * 23 + x * 7).unwrap(),
            u8::try_from(47 + row * 13 + x * 5).unwrap(),
            u8::try_from(0x80 + row * 9 + x * 3).unwrap(),
        ]);
    }
    out
}

fn encode_png_filtered_row(filter: u8, decoded: &[u8], previous: &[u8], bpp: usize) -> Vec<u8> {
    decoded
        .iter()
        .copied()
        .enumerate()
        .map(|(i, value)| {
            let left = i
                .checked_sub(bpp)
                .and_then(|index| decoded.get(index))
                .copied()
                .unwrap_or(0);
            let up = previous.get(i).copied().unwrap_or(0);
            let up_left = i
                .checked_sub(bpp)
                .and_then(|index| previous.get(index))
                .copied()
                .unwrap_or(0);
            let prediction = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                4 => test_png_paeth(left, up, up_left),
                _ => unreachable!("test only emits PNG filters 0..=4"),
            };
            value.wrapping_sub(prediction)
        })
        .collect()
}

fn test_png_paeth(left: u8, up: u8, up_left: u8) -> u8 {
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

fn minimal_png_rgba_adam7() -> Vec<u8> {
    const WIDTH: usize = 8;
    const HEIGHT: usize = 8;
    const ADAM7_PASSES: [(usize, usize, usize, usize); 7] = [
        (0, 0, 8, 8),
        (4, 0, 8, 8),
        (0, 4, 4, 8),
        (2, 0, 4, 4),
        (0, 2, 2, 4),
        (1, 0, 2, 2),
        (0, 1, 1, 2),
    ];
    let mut raw = Vec::new();
    for (x_start, y_start, x_step, y_step) in ADAM7_PASSES {
        let pass_width = if WIDTH <= x_start {
            0
        } else {
            (WIDTH - x_start).div_ceil(x_step)
        };
        let pass_height = if HEIGHT <= y_start {
            0
        } else {
            (HEIGHT - y_start).div_ceil(y_step)
        };
        for pass_y in 0..pass_height {
            raw.push(0);
            let y = y_start + pass_y * y_step;
            for pass_x in 0..pass_width {
                let x = x_start + pass_x * x_step;
                raw.extend_from_slice(&adam7_test_pixel(x, y));
            }
        }
    }
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 8, 0, 0, 0, 8, 8, 6, 0, 0, 1]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn adam7_test_pixel(x: usize, y: usize) -> [u8; 4] {
    [
        u8::try_from(x * 17).unwrap(),
        u8::try_from(y * 19).unwrap(),
        u8::try_from(0x80 + x + y).unwrap(),
        0xFF,
    ]
}

fn minimal_png_rgba_with_idat(
    width: u32,
    height: u32,
    encode_idat: fn(&[u8]) -> Vec<u8>,
) -> Vec<u8> {
    let mut raw = Vec::new();
    for _ in 0..height {
        raw.resize(raw.len() + 1, 0);
        for _ in 0..width {
            raw.extend_from_slice(&[0x11, 0x22, 0x33, 0xFF]);
        }
    }
    let idat = encode_idat(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    append_png_chunk(&mut data, b"IHDR", &ihdr);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn minimal_png_indexed_2bit() -> Vec<u8> {
    let raw = [0, 0b00_01_10_11];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 4, 0, 0, 0, 1, 2, 3, 0, 0, 0]);
    append_png_chunk(
        &mut data,
        b"PLTE",
        &[
            0xFF, 0x00, 0x00, // red
            0x00, 0xFF, 0x00, // green
            0x00, 0x00, 0xFF, // blue
            0xFF, 0xFF, 0x00, // transparent yellow
        ],
    );
    append_png_chunk(&mut data, b"tRNS", &[0xFF, 0xFF, 0xFF, 0x00]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn minimal_png_grayscale_1bit_trns() -> Vec<u8> {
    let raw = [0, 0b0100_0000];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 1, 1, 0, 0, 0, 0]);
    append_png_chunk(&mut data, b"tRNS", &[0, 1]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn minimal_png_grayscale_alpha_8bit() -> Vec<u8> {
    let raw = [0, 0x44, 0x80, 0xCC, 0xFF];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 1, 8, 4, 0, 0, 0]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn minimal_png_rgb_8bit_trns() -> Vec<u8> {
    let raw = [0, 0x11, 0x22, 0x33, 0xAA, 0xBB, 0xCC];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 1, 8, 2, 0, 0, 0]);
    append_png_chunk(&mut data, b"tRNS", &[0, 0x11, 0, 0x22, 0, 0x33]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn minimal_png_grayscale_alpha_16bit() -> Vec<u8> {
    let raw = [
        0, // filter
        0x12, 0x34, // gray -> 0x12
        0x80, 0x90, // alpha -> 0x80
        0xDE, 0xAD, // gray -> 0xDE
        0x7F, 0x00, // alpha -> 0x7F
    ];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 1, 16, 4, 0, 0, 0]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn overwide_png_rgb_16bit() -> Vec<u8> {
    let raw = [
        0, // filter
        0x12, 0x34, 0xAB, 0xCD, 0x56, 0x78, // pixel 0
        0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, // pixel 1
    ];
    let idat = zlib_stored_block(&raw);
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1A\n");
    append_png_chunk(&mut data, b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 1, 16, 2, 0, 0, 0]);
    append_png_chunk(&mut data, b"IDAT", &idat);
    append_png_chunk(&mut data, b"IEND", &[]);
    data
}

fn png_with_corrupt_idat_crc() -> Vec<u8> {
    let mut data = minimal_png_rgba(1, 1);
    let mut offset = 8usize;
    while offset + 12 <= data.len() {
        let len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        let chunk_type_start = offset + 4;
        let chunk_data_end = offset + 8 + len;
        let chunk_end = chunk_data_end + 4;
        if &data[chunk_type_start..chunk_type_start + 4] == b"IDAT" {
            data[chunk_end - 1] ^= 0x01;
            return data;
        }
        offset = chunk_end;
    }
    panic!("test PNG helper must contain an IDAT chunk");
}

fn append_png_chunk(data: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    data.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    data.extend_from_slice(kind);
    data.extend_from_slice(payload);
    let mut crc_input = Vec::with_capacity(4 + payload.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(payload);
    data.extend_from_slice(&test_png_crc32(&crc_input).to_be_bytes());
}

fn test_png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn zlib_stored_block(raw: &[u8]) -> Vec<u8> {
    assert!(raw.len() <= u16::MAX as usize);
    let mut out = vec![0x78, 0x01, 0x01];
    let len = raw.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(raw);
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn zlib_fixed_literal_block(raw: &[u8]) -> Vec<u8> {
    let mut writer = DeflateBitWriter::default();
    writer.write_bits_lsb(0b011, 3);
    for &byte in raw {
        writer.write_fixed_literal(byte);
    }
    writer.write_fixed_end_of_block();
    let deflate = writer.finish();
    let mut out = vec![0x78, 0x01];
    out.extend_from_slice(&deflate);
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn zlib_dynamic_literal_block(raw: &[u8]) -> Vec<u8> {
    let mut literal_lengths = vec![0u8; 257];
    for &byte in raw {
        literal_lengths[usize::from(byte)] = 3;
    }
    literal_lengths[256] = 3;

    let mut writer = DeflateBitWriter::default();
    writer.write_bits_lsb(0b101, 3); // BFINAL=1, BTYPE=dynamic Huffman.
    writer.write_bits_lsb(0, 5); // HLIT: 257 literal/length codes.
    writer.write_bits_lsb(0, 5); // HDIST: 1 distance code, unused here.
    writer.write_bits_lsb(10, 4); // HCLEN: first 14 code-length-code slots.

    let mut code_length_lengths = vec![0u8; 19];
    code_length_lengths[0] = 1;
    code_length_lengths[3] = 1;
    for &symbol in &[16usize, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3] {
        writer.write_bits_lsb(u16::from(code_length_lengths[symbol]), 3);
    }

    let code_length_codes = huffman_codes_from_lengths(&code_length_lengths);
    for &length in &literal_lengths {
        write_huffman_symbol(&mut writer, &code_length_codes, usize::from(length));
    }
    write_huffman_symbol(&mut writer, &code_length_codes, 0); // unused distance code.

    let literal_codes = huffman_codes_from_lengths(&literal_lengths);
    for &byte in raw {
        write_huffman_symbol(&mut writer, &literal_codes, usize::from(byte));
    }
    write_huffman_symbol(&mut writer, &literal_codes, 256);

    let deflate = writer.finish();
    let mut out = vec![0x78, 0x01];
    out.extend_from_slice(&deflate);
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn zlib_dynamic_backref_block(raw: &[u8]) -> Vec<u8> {
    assert_eq!(
        raw.len(),
        18,
        "this test encoder expects two identical 2x2 RGBA PNG scanlines"
    );
    let (first_scanline, second_scanline) = raw.split_at(9);
    assert_eq!(first_scanline, second_scanline);

    let mut literal_lengths = vec![0u8; 264];
    for &byte in first_scanline {
        literal_lengths[usize::from(byte)] = 3;
    }
    literal_lengths[256] = 3; // end of block
    literal_lengths[263] = 3; // length 9, no extra bits

    let mut distance_lengths = vec![0u8; 7];
    distance_lengths[6] = 3; // distance 9, two zero extra bits

    let mut writer = DeflateBitWriter::default();
    writer.write_bits_lsb(0b101, 3); // BFINAL=1, BTYPE=dynamic Huffman.
    writer.write_bits_lsb(7, 5); // HLIT: 264 literal/length codes.
    writer.write_bits_lsb(6, 5); // HDIST: 7 distance codes.
    writer.write_bits_lsb(10, 4); // HCLEN: first 14 code-length-code slots.

    let mut code_length_lengths = vec![0u8; 19];
    code_length_lengths[0] = 1;
    code_length_lengths[3] = 1;
    for &symbol in &[16usize, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3] {
        writer.write_bits_lsb(u16::from(code_length_lengths[symbol]), 3);
    }

    let code_length_codes = huffman_codes_from_lengths(&code_length_lengths);
    for &length in &literal_lengths {
        write_huffman_symbol(&mut writer, &code_length_codes, usize::from(length));
    }
    for &length in &distance_lengths {
        write_huffman_symbol(&mut writer, &code_length_codes, usize::from(length));
    }

    let literal_codes = huffman_codes_from_lengths(&literal_lengths);
    let distance_codes = huffman_codes_from_lengths(&distance_lengths);
    for &byte in first_scanline {
        write_huffman_symbol(&mut writer, &literal_codes, usize::from(byte));
    }
    write_huffman_symbol(&mut writer, &literal_codes, 263);
    write_huffman_symbol(&mut writer, &distance_codes, 6);
    writer.write_bits_lsb(0, 2);
    write_huffman_symbol(&mut writer, &literal_codes, 256);

    let deflate = writer.finish();
    let mut out = vec![0x78, 0x01];
    out.extend_from_slice(&deflate);
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

fn huffman_codes_from_lengths(lengths: &[u8]) -> Vec<Option<(u16, u8)>> {
    let mut counts = [0u16; 16];
    for &len in lengths {
        if len != 0 {
            counts[usize::from(len)] += 1;
        }
    }
    let mut code = 0u16;
    let mut next_code = [0u16; 16];
    for bits in 1..=15 {
        code = (code + counts[bits - 1]) << 1;
        next_code[bits] = code;
    }
    let mut codes = vec![None; lengths.len()];
    for (symbol, &len) in lengths.iter().enumerate() {
        if len == 0 {
            continue;
        }
        let code = next_code[usize::from(len)];
        next_code[usize::from(len)] += 1;
        codes[symbol] = Some((code, len));
    }
    codes
}

fn write_huffman_symbol(writer: &mut DeflateBitWriter, codes: &[Option<(u16, u8)>], symbol: usize) {
    let (code, bits) = codes[symbol].expect("test dynamic deflate symbol has a code");
    writer.write_huffman_bits_msb(code, bits);
}

#[derive(Default)]
struct DeflateBitWriter {
    bytes: Vec<u8>,
    bit_offset: usize,
}

impl DeflateBitWriter {
    fn write_bit(&mut self, bit: u8) {
        if self.bit_offset.is_multiple_of(8) {
            self.bytes.push(0);
        }
        if bit & 1 != 0 {
            let index = self.bytes.len() - 1;
            self.bytes[index] |= 1 << (self.bit_offset % 8);
        }
        self.bit_offset += 1;
    }

    fn write_bits_lsb(&mut self, value: u16, bits: u8) {
        for bit in 0..bits {
            self.write_bit(((value >> bit) & 1) as u8);
        }
    }

    fn write_huffman_bits_msb(&mut self, value: u16, bits: u8) {
        for bit in (0..bits).rev() {
            self.write_bit(((value >> bit) & 1) as u8);
        }
    }

    fn write_fixed_literal(&mut self, byte: u8) {
        let value = u16::from(byte);
        match value {
            0..=143 => self.write_huffman_bits_msb(0b0011_0000 + value, 8),
            144..=255 => self.write_huffman_bits_msb(0b1_1001_0000 + (value - 144), 9),
            _ => unreachable!(),
        }
    }

    fn write_fixed_end_of_block(&mut self) {
        self.write_huffman_bits_msb(0, 7);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
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

fn render_with(
    pool: &ObjectPool,
    engine: &LayoutEngine,
    active_mask: ObjectID,
) -> machbus::isobus::vt::render::Scene {
    engine.build(pool, active_mask)
}

fn fixed_vt_command(command: u8) -> Vec<u8> {
    let mut data = [0xFFu8; 8];
    data[0] = command;
    data.to_vec()
}

fn parse_named_hex_bytes(fixtures: &str, name: &str) -> Vec<u8> {
    let line = fixtures
        .lines()
        .map(|line| line.split('#').next().unwrap_or("").trim())
        .filter(|line| !line.is_empty())
        .find(|line| {
            line.split_once('=')
                .is_some_and(|(key, _)| key.trim() == name)
        })
        .unwrap_or_else(|| panic!("fixture {name} not found"));
    let (_, hex) = line.split_once('=').unwrap();
    let hex = hex.trim();
    assert_eq!(hex.len() % 2, 0, "fixture {name} has odd hex length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

// ─── Pipeline: load bytes → validate → scene ───────────────────────

#[test]
fn render_pipeline_loads_validates_and_builds_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap());
    let bytes = pool.serialize().unwrap();

    let doc = IopDocument::load(&bytes, DocConfig::default()).expect("valid pool loads");
    assert_eq!(doc.pool().size(), 3);
    // Initial mask = first working-set child.
    assert_eq!(doc.scene().active_mask, ObjectID::new(2));
    // One drawable child + mask background emitted.
    assert!(!doc.scene().nodes.is_empty());
}

#[test]
fn render_pipeline_loads_reviewable_object_pool_fixture_and_framebuffer() {
    let bytes = parse_named_hex_bytes(VT_OBJECT_POOL_HEX, "valid_ws_datamask");
    let doc = IopDocument::load(&bytes, DocConfig::default()).expect("fixture pool loads");

    assert_eq!(doc.pool().size(), 2);
    assert_eq!(doc.scene().active_mask, ObjectID::new(2));
    assert!(doc.scene().unsupported.is_empty());

    let commands = GtuiRenderer::default().render(doc.scene());
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::FillRect { .. })),
        "fixture should lower to drawable backend commands"
    );
    let framebuffer = FramebufferRenderer::default()
        .render_scene(doc.scene())
        .expect("fixture scene renders to framebuffer");
    assert_eq!(framebuffer.width(), DocConfig::default().canvas.0);
    assert_eq!(framebuffer.height(), DocConfig::default().canvas.1);
}

#[test]
fn iop_document_loads_external_vt3_test_pool_fixture() {
    let doc = IopDocument::load(VT3_TEST_POOL_IOP, DocConfig::default())
        .expect("external VT3 test pool fixture loads");

    assert_eq!(doc.pool().size(), 34);
    assert_eq!(doc.scene().active_mask, ObjectID::new(1000));
    assert!(
        doc.scene().unsupported.is_empty(),
        "promoted external basic fixture must stay free of unsupported records"
    );
    let framebuffer = FramebufferRenderer::default()
        .render_scene(doc.scene())
        .expect("external VT3 test pool renders to framebuffer");
    assert_eq!(framebuffer.width(), 544);
    assert_eq!(framebuffer.height(), 480);
}

#[test]
fn vt_server_change_active_mask_accepts_external_alarm_mask_fixture() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    let source = 0x42;
    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_vt_command(cmd::GET_MEMORY),
                source,
            ))
            .len(),
        1
    );
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend_from_slice(VT3_TEST_POOL_IOP);
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, source))
            .is_empty()
    );
    let end_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_vt_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(end_response.len(), 1);
    assert_eq!(end_response[0].data[1], 0x00);

    let mut change_active_mask = [0xFFu8; 8];
    change_active_mask[0] = cmd::CHANGE_ACTIVE_MASK;
    change_active_mask[1..3].copy_from_slice(&0u16.to_le_bytes());
    change_active_mask[3..5].copy_from_slice(&0x07D0u16.to_le_bytes());
    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                change_active_mask.to_vec(),
                source,
            ))
            .is_empty()
    );

    let client = &server.clients()[0];
    assert_eq!(client.object_state.active_data_mask, ObjectID::new(0x07D0));
    assert!(matches!(
        client.object_state.accepted_effects.last(),
        Some(ServerRenderEffect::ChangeActiveMask { mask }) if *mask == ObjectID::new(0x07D0)
    ));
    let runtime = VtRenderRuntime::from_server_working_set(
        client,
        LayoutConfig {
            physical_soft_key_count: 10,
            navigation_soft_key_count: 2,
            ..LayoutConfig::default()
        },
    )
    .expect("external alarm-mask runtime snapshot builds");
    let framebuffer = FramebufferRenderer::default()
        .try_render_runtime(&runtime)
        .expect("external alarm-mask runtime renders");
    assert_eq!(framebuffer.width(), 544);
    assert_eq!(framebuffer.height(), 420);
}

#[test]
fn vt_external_evidence_manifest_keeps_phase10_requirements_explicit() {
    let mut ids = Vec::new();
    let mut rows = 0usize;
    for raw_line in VT_EXTERNAL_EVIDENCE_REQUIREMENTS.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        rows += 1;
        let fields: Vec<_> = line.split('|').collect();
        assert_eq!(
            fields.len(),
            7,
            "VT external evidence row must have 7 pipe-separated fields: {line}"
        );
        let id = fields[0];
        assert!(!id.is_empty(), "VT external evidence id may not be empty");
        assert!(
            fields[0]
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
            "VT external evidence id must be stable snake_case: {id}"
        );
        assert!(
            !fields[1].is_empty(),
            "VT external evidence scope may not be empty for {id}"
        );
        assert!(
            ids.iter().all(|existing| *existing != id),
            "duplicate VT external evidence id: {id}"
        );
        ids.push(id);
        assert_eq!(
            fields[3], "independent-tool-or-open-license",
            "VT external evidence source class for {id} must stay redistributable/auditable"
        );
        assert!(
            !fields[6].is_empty(),
            "VT external evidence notes may not be empty for {id}"
        );
        assert!(
            matches!(fields[2], "missing" | "complete"),
            "invalid VT external evidence status for {id}: {}",
            fields[2]
        );
        if fields[2] == "missing" {
            assert_eq!(
                fields[4], "-",
                "missing VT evidence row {id} must not claim artifacts"
            );
            assert_eq!(
                fields[5], "-",
                "missing VT evidence row {id} must not claim a report"
            );
        } else {
            assert_ne!(
                fields[4], "-",
                "complete VT evidence row {id} must name promoted artifacts"
            );
            assert!(
                fields[4]
                    .split('+')
                    .any(|artifact| artifact.starts_with("tests/fixtures/isobus/")),
                "complete VT evidence row {id} must name a checked-in isobus fixture artifact"
            );
            assert!(
                fields[4]
                    .split('+')
                    .any(|artifact| artifact.ends_with(".json")),
                "complete VT evidence row {id} must name an inspector JSON artifact"
            );
            for artifact in fields[4].split('+') {
                assert!(
                    std::path::Path::new(artifact).exists(),
                    "complete VT evidence row {id} artifact missing: {artifact}"
                );
            }
            assert!(
                fields[5].starts_with("tests/fixtures/isobus/vt_external_reports/"),
                "complete VT evidence row {id} must name a report under vt_external_reports"
            );
            assert!(
                fields[5].ends_with(".md"),
                "complete VT evidence row {id} must name a Markdown report"
            );
            let report = std::fs::read_to_string(fields[5]).unwrap_or_else(|error| {
                panic!("complete VT evidence row {id} report missing: {error}")
            });
            assert!(
                report.contains(&format!("Requirement: {id}")),
                "complete VT evidence report for {id} must cite the matching requirement id"
            );
            assert!(
                report.contains("Fixture path:")
                    && report.contains("Fixture hash:")
                    && report.contains("Source:")
                    && report.contains("License / redistribution basis:")
                    && report.contains("Acquired:")
                    && report.contains("--write-report-json")
                    && report.contains("artifacts")
                    && report.contains("report_json")
                    && report.contains("--expect-unsupported-records")
                    && report.contains("--expect-placeholder-pixels")
                    && report.contains("rgb888_fnv64")
                    && report.contains("rgb565_be_fnv64")
                    && report.contains("rgb565_le_fnv64")
                    && report.contains("pool-buffer hash")
                    && report.contains("layout profile")
                    && report.contains("Caveats and non-claims:"),
                "complete VT evidence report for {id} is missing required provenance/check fields"
            );
            if id == "vt_external_command_trace" {
                assert!(
                    report.contains("vt_trace_inspect")
                        && report.contains("--trace")
                        && report.contains("--expect-accepted-effects")
                        && report.contains("--expect-initial-placeholder-pixels")
                        && report.contains("--expect-final-placeholder-pixels")
                        && report.contains("trace payload hash")
                        && report.contains("accepted-effect"),
                    "command-trace evidence report must include trace command provenance"
                );
            }
        }
    }

    assert!(
        rows >= 6,
        "Phase 10 must keep independent pool categories explicit"
    );
    for required in [
        "vt_external_pool_basic",
        "vt_external_pool_graphics",
        "vt_external_pool_soft_keys",
        "vt_external_pool_inputs",
        "vt_external_pool_user_layout",
        "vt_external_command_trace",
    ] {
        assert!(
            ids.contains(&required),
            "missing VT external evidence requirement id: {required}"
        );
        assert!(
            VT_EXTERNAL_REPORTS_README.contains(required)
                || VT_EXTERNAL_REPORTS_README.contains("<vt_external_evidence_requirements id>"),
            "report README must document how to cite requirement ids"
        );
    }
}

#[test]
fn vt_protocol_matrix_reflects_promoted_phase10_evidence() {
    let vt_render_row = PROTOCOL_MATRIX_CSV
        .lines()
        .find(|line| {
            line.starts_with("ISO11783-6,src/isobus/vt/render,Virtual Terminal render runtime,")
        })
        .expect("protocol matrix must keep a VT render runtime row");

    assert!(
        vt_render_row.contains("promoted VT3")
            && vt_render_row.contains("reduced seeder")
            && vt_render_row.contains("reduced VT3 command trace")
            && vt_render_row.contains("RGB888/RGB565 artifacts"),
        "VT render protocol-matrix row must describe the promoted Phase 10 pool and trace evidence"
    );
    assert!(
        !vt_render_row.contains("still needs independent .iop pools"),
        "VT render protocol-matrix row must not describe closed Phase 10 pool evidence as missing"
    );
    assert!(
        vt_render_row
            .contains("broader target-display/commercial-VT comparisons remain future evidence"),
        "VT render row should keep the remaining backend/display evidence gap explicit"
    );
}

#[test]
fn vt_external_evidence_report_template_mentions_all_phase10_gates() {
    for required in [
        "machbus-iop-inspect-report-v1",
        "machbus-vt-trace-inspect-report-v1",
        "iop_inspect",
        "vt_trace_inspect",
        "--strict",
        "--active-mask",
        "--write-report-json",
        "--write-initial-rgb888",
        "--write-final-rgb888",
        "--write-initial-rgb565-be",
        "--write-initial-rgb565-le",
        "--write-final-rgb565-be",
        "--write-final-rgb565-le",
        "--expect-unsupported-records",
        "--expect-placeholder-pixels",
        "--expect-accepted-effects",
        "--expect-initial-placeholder-pixels",
        "--expect-final-placeholder-pixels",
        "--expect-rgb888-fnv64",
        "--expect-rgb565-be-fnv64",
        "--expect-rgb565-le-fnv64",
        "artifacts",
        "report_json",
        "initial_rgb888",
        "final_rgb565_le",
        "rgb565_be_fnv64",
        "rgb565_le_fnv64",
        "--canvas",
        "--soft-key-area",
        "--physical-soft-keys",
        "--navigation-soft-keys",
        "--soft-key-page",
        "--pool-fixture",
        "pool-buffer hash",
        "trace payload hash",
        "unsupported records",
        "placeholder pixels",
        "Caveats and non-claims",
    ] {
        assert!(
            VT_EXTERNAL_REPORTS_README.contains(required),
            "VT external report README must document Phase 10 gate `{required}`"
        );
    }
}

#[test]
fn vt_evidence_smoke_target_archives_static_and_trace_framebuffer_artifacts() {
    let Some((_, smoke_target)) = PROJECT_MAKEFILE.split_once("vt-evidence-smoke:") else {
        panic!("Makefile must define vt-evidence-smoke");
    };
    let smoke_target = smoke_target
        .split("\nfuzz-smoke:")
        .next()
        .expect("vt-evidence-smoke body must precede fuzz-smoke");

    for required in [
        "--example iop_inspect",
        "--write-report-json /tmp/machbus-iop-inspect-report.json",
        "--write-rgb888 /tmp/machbus-iop-inspect.rgb",
        "--write-rgb565-be /tmp/machbus-iop-inspect-be.rgb565",
        "--write-rgb565-le /tmp/machbus-iop-inspect-le.rgb565",
        "--expect-unsupported-records 0",
        "--expect-placeholder-pixels 0",
        "--expect-rgb888-fnv64 0x527FEA44D2914422",
        "--expect-rgb565-be-fnv64 0xC0F7DB231D7BC71F",
        "--expect-rgb565-le-fnv64 0xA27374387E955487",
        "--example vt_trace_inspect",
        "--write-initial-rgb888 /tmp/machbus-vt-trace-initial.rgb",
        "--write-final-rgb888 /tmp/machbus-vt-trace-final.rgb",
        "--write-initial-rgb565-be /tmp/machbus-vt-trace-initial-be.rgb565",
        "--write-initial-rgb565-le /tmp/machbus-vt-trace-initial-le.rgb565",
        "--write-final-rgb565-be /tmp/machbus-vt-trace-final-be.rgb565",
        "--write-final-rgb565-le /tmp/machbus-vt-trace-final-le.rgb565",
        "--expect-accepted-effects 3",
        "--expect-initial-placeholder-pixels 0",
        "--expect-final-placeholder-pixels 0",
    ] {
        assert!(
            smoke_target.contains(required),
            "vt-evidence-smoke must preserve Phase 10 evidence gate `{required}`"
        );
    }
}

#[test]
fn render_pipeline_never_panics_on_malformed_bytes() {
    // Truncated header.
    assert!(IopDocument::load(&[0x01, 0x00], DocConfig::default()).is_err());
    // Unknown object type byte.
    let bad = [0x01, 0x00, 0xFF, 0x00, 0x00];
    assert!(IopDocument::load(&bad, DocConfig::default()).is_err());
    // Empty input.
    assert!(IopDocument::load(&[], DocConfig::default()).is_err());
}

#[test]
fn render_pipeline_rejects_pool_without_working_set() {
    let pool = ObjectPool::default().with_object(create_data_mask(2, &DataMaskBody::default()));
    let bytes = pool.serialize().unwrap();
    assert!(IopDocument::load(&bytes, DocConfig::default()).is_err());
}

// ─── Masks / containers / basic shapes ─────────────────────────────

#[test]
fn render_data_mask_emits_background_and_child_nodes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    let scene = render(&pool, ObjectID::NULL);
    let drawables = scene
        .nodes
        .iter()
        .filter(|n| n.object_type == ObjectType::OutputString)
        .count();
    assert_eq!(drawables, 2);
}

#[test]
fn render_alarm_mask_is_supported_as_active_mask() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_alarm_mask(2, &AlarmMaskBody::default()).unwrap());
    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(scene.active_mask, ObjectID::new(2));
}

#[test]
fn render_container_recurses_into_children() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_container(
                3,
                &ContainerBody {
                    width: 100,
                    height: 100,
                    hidden: false,
                },
            )
            .with_children([4u16]),
        )
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    let scene = render(&pool, ObjectID::NULL);
    // Container + its grandchild must both appear.
    assert!(scene.nodes.iter().any(|n| n.id == 3));
    assert!(scene.nodes.iter().any(|n| n.id == 4));
}

#[test]
fn render_object_pointer_materialises_target_and_numeric_value_retargets_it() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(30),
                25,
                35,
            )]),
        )
        .with_object(create_object_pointer(
            30,
            &ObjectPointerBody {
                value: ObjectID::new(31),
            },
        ))
        .with_object(
            create_output_string(
                31,
                &OutputStringBody {
                    width: 40,
                    height: 12,
                    value: b"ONE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                32,
                &OutputStringBody {
                    width: 50,
                    height: 14,
                    value: b"TWO".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert!(
        scene.find(ObjectID::new(30)).is_none(),
        "ObjectPointer is an indirection, not its own visible node"
    );
    let pointed = scene.find(ObjectID::new(31)).unwrap();
    assert_eq!(pointed.rect, Rect::new(25, 35, 40, 12));
    assert!(matches!(
        &pointed.kind,
        NodeKind::OutputString { text, .. } if text == "ONE"
    ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(30),
                attribute_id: 1,
                value: 32,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "ObjectPointer exposes its target value for Get Attribute Value, but the standard retargeting command is Change Numeric Value"
    );
    assert!(runtime.scene().find(ObjectID::new(32)).is_none());

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(30),
            value: 32,
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert!(runtime.scene().find(ObjectID::new(31)).is_none());
    let numeric_retargeted = runtime.scene().find(ObjectID::new(32)).unwrap();
    assert_eq!(numeric_retargeted.rect, Rect::new(25, 35, 50, 14));
    assert!(matches!(
        &numeric_retargeted.kind,
        NodeKind::OutputString { text, .. } if text == "TWO"
    ));
}

#[test]
fn render_button_label_resolves_child_output_text() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(
            create_button(
                30,
                &ButtonBody {
                    width: 64,
                    height: 24,
                    key_code: 12,
                    ..Default::default()
                },
            )
            .with_children([31u16]),
        )
        .with_object(create_object_pointer(
            31,
            &ObjectPointerBody {
                value: ObjectID::new(32),
            },
        ))
        .with_object(
            create_output_string(
                32,
                &OutputStringBody {
                    value: b"Push".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    let button = scene.find(ObjectID::new(30)).unwrap();
    assert!(matches!(
        &button.kind,
        NodeKind::Button {
            label,
            enabled: true,
            key_number,
            ..
        } if label == "Push" && *key_number == 12
    ));
    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Push")
        )
    );
}

#[test]
fn render_button_uses_background_border_and_transparency_options() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([
                ChildRef::new(ObjectID::new(30), 0, 0),
                ChildRef::new(ObjectID::new(31), 60, 0),
            ]),
        )
        .with_object(create_button(
            30,
            &ButtonBody {
                width: 40,
                height: 20,
                background_color: 2,
                border_color: 3,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            31,
            &ButtonBody {
                width: 40,
                height: 20,
                background_color: 4,
                border_color: 5,
                options: 0x28,
                ..Default::default()
            },
        ));

    let scene = render(&pool, ObjectID::NULL);
    let first = scene.find(ObjectID::new(30)).unwrap();
    assert_eq!(first.style.background, Palette::default().resolve(2));
    assert_eq!(first.style.foreground, Palette::default().resolve(3));
    assert!(matches!(
        &first.kind,
        NodeKind::Button {
            transparent_bg: false,
            draw_border: true,
            ..
        }
    ));
    let second = scene.find(ObjectID::new(31)).unwrap();
    assert!(matches!(
        &second.kind,
        NodeKind::Button {
            transparent_bg: true,
            draw_border: false,
            ..
        }
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, colour }
            if *rect == first.rect && *colour == Palette::default().resolve(2)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::StrokeRect { rect, colour, .. }
            if *rect == first.rect && *colour == Palette::default().resolve(3)
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, .. } | RenderCommand::StrokeRect { rect, .. }
            if *rect == second.rect
    )));
}

#[test]
fn render_text_fields_use_full_justification_and_transparent_background_options() {
    let input_string_rect = Rect::new(10, 10, 80, 24);
    let input_number_rect = Rect::new(10, 40, 80, 24);
    let output_string_rect = Rect::new(10, 70, 80, 24);
    let output_number_rect = Rect::new(10, 100, 80, 24);
    let wrapping_input_string_rect = Rect::new(100, 10, 20, 40);
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([
                ChildRef::new(ObjectID::new(10), 10, 10),
                ChildRef::new(ObjectID::new(11), 10, 40),
                ChildRef::new(ObjectID::new(12), 10, 70),
                ChildRef::new(ObjectID::new(13), 10, 100),
                ChildRef::new(ObjectID::new(14), 100, 10),
            ]),
        )
        .with_object(
            create_input_string(
                10,
                &InputStringBody {
                    width: 80,
                    height: 24,
                    background_color: 2,
                    options: 0x01,
                    variable_reference: ObjectID::new(20),
                    justification: 0x0A,
                    max_length: 8,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                11,
                &InputNumberBody {
                    width: 80,
                    height: 24,
                    background_color: 3,
                    options: 0x00,
                    value: 42,
                    min_value: 0,
                    max_value: 100,
                    justification: 0x05,
                    options2: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                12,
                &OutputStringBody {
                    width: 80,
                    height: 24,
                    background_color: 4,
                    options: 0x01,
                    justification: 0x08,
                    value: b"out".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_number(
                13,
                &OutputNumberBody {
                    width: 80,
                    height: 24,
                    background_color: 5,
                    options: 0x00,
                    value: 7,
                    justification: 0x02,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                14,
                &InputStringBody {
                    width: 20,
                    height: 40,
                    options: 0x02,
                    variable_reference: ObjectID::new(21),
                    justification: 0,
                    max_length: 8,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            20,
            &StringVariableBody {
                length: 8,
                value: b"abc".to_vec(),
            },
        ))
        .with_object(create_string_variable(
            21,
            &StringVariableBody {
                length: 8,
                value: b"abcd".to_vec(),
            },
        ));

    let scene = render(&pool, ObjectID::NULL);
    assert!(matches!(
        &scene.find(ObjectID::new(10)).unwrap().kind,
        NodeKind::InputString {
            transparent_bg: true,
            justification: 0x0A,
            ..
        }
    ));
    assert!(matches!(
        &scene.find(ObjectID::new(11)).unwrap().kind,
        NodeKind::InputNumber {
            transparent_bg: false,
            justification: 0x05,
            ..
        }
    ));
    assert!(matches!(
        &scene.find(ObjectID::new(13)).unwrap().kind,
        NodeKind::OutputNumber {
            transparent_bg: false,
            justification: 0x02,
            ..
        }
    ));
    assert!(matches!(
        &scene.find(ObjectID::new(14)).unwrap().kind,
        NodeKind::InputString {
            auto_wrap: true,
            ..
        }
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, .. } if *rect == input_string_rect
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, .. } if *rect == output_string_rect
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, colour }
            if *rect == input_number_rect && *colour == Palette::default().resolve(3)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, colour }
            if *rect == output_number_rect && *colour == Palette::default().resolve(5)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText {
            rect,
            text,
            align,
            layout,
            ..
        } if *rect == input_string_rect
            && text == "abc"
            && *align == HorizontalAlign::Right
            && layout.vertical_align == VerticalAlign::Bottom
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText {
            rect,
            text,
            align,
            layout,
            ..
        } if *rect == input_number_rect
            && text == "42"
            && *align == HorizontalAlign::Middle
            && layout.vertical_align == VerticalAlign::Middle
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText {
            rect,
            text,
            align,
            layout,
            ..
        } if *rect == output_string_rect
            && text == "out"
            && *align == HorizontalAlign::Left
            && layout.vertical_align == VerticalAlign::Bottom
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText {
            rect,
            text,
            align,
            layout,
            ..
        } if *rect == output_number_rect
            && text == "7"
            && *align == HorizontalAlign::Right
            && layout.vertical_align == VerticalAlign::Top
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText {
            rect,
            text,
            layout,
            ..
        } if *rect == wrapping_input_string_rect
            && text == "ab\ncd"
            && layout.lines.len() == 2
            && layout.clipped_rows == 0
    )));
}

#[test]
fn render_basic_shapes_resolve_rect_line_ellipse() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16, 6u16]),
        )
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 50,
                    height: 50,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_line(
                4,
                &OutputLineBody {
                    width: 40,
                    height: 40,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_ellipse(
                5,
                &OutputEllipseBody {
                    width: 30,
                    height: 30,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_fill_attributes(
                12,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 18,
                    fill_pattern: ObjectID::NULL,
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let renderer = GtuiRenderer::default();
    let cmds = renderer.render(&scene);
    // Rectangle, line, and ellipse all emit draw commands.
    assert!(
        cmds.iter()
            .any(|c| matches!(c, RenderCommand::StrokeRect { .. }))
    );
    assert!(cmds.iter().any(|c| matches!(c, RenderCommand::Line { .. })));
    assert!(
        cmds.iter()
            .any(|c| matches!(c, RenderCommand::Ellipse { .. }))
    );
}

#[test]
fn render_output_shape_line_width_zero_suppresses_shape_strokes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16, 6u16]),
        )
        .with_object(create_line_attributes(
            10,
            &LineAttributesBody {
                line_color: 1,
                line_width: 0,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_fill_attributes(
                11,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 18,
                    fill_pattern: ObjectID::NULL,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_line(
                3,
                &OutputLineBody {
                    width: 20,
                    height: 20,
                    line_attributes: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                4,
                &OutputRectangleBody {
                    width: 20,
                    height: 20,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_ellipse(
                5,
                &OutputEllipseBody {
                    width: 20,
                    height: 20,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_polygon(
                6,
                &OutputPolygonBody {
                    width: 20,
                    height: 20,
                    line_attributes: ObjectID::new(10),
                    fill_attributes: ObjectID::new(11),
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 20, y: 0 },
                        PolygonPoint { x: 20, y: 20 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    for id in [3, 4, 5, 6] {
        assert_eq!(
            scene.find(ObjectID::new(id)).unwrap().style.line_width,
            0,
            "explicit LineAttributes width 0 must be retained for object {id}"
        );
    }
    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Line { .. })),
        "line-width-zero Output Line must not emit a stroke command"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, RenderCommand::StrokeRect { .. })),
        "line-width-zero rectangle must not emit an outline stroke"
    );
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { colour, .. } if *colour == Palette::default_isobus().resolve(18)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ellipse {
            filled: true,
            width: 0,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Polygon {
            filled: true,
            width: 0,
            ..
        }
    )));
}

#[test]
fn framebuffer_treats_zero_width_line_commands_as_noops() {
    let black = Colour::rgb(0, 0, 0);
    let framebuffer = FramebufferRenderer::default()
        .render_commands(
            8,
            8,
            &[
                RenderCommand::Line {
                    x0: 0,
                    y0: 0,
                    x1: 7,
                    y1: 7,
                    colour: black,
                    width: 0,
                    line_art: 0xFFFF,
                },
                RenderCommand::StrokeRect {
                    rect: Rect::new(0, 0, 8, 8),
                    colour: black,
                    width: 0,
                    line_art: 0xFFFF,
                    suppress: 0,
                },
            ],
        )
        .unwrap();

    assert_eq!(framebuffer.count_colour(black), 0);
}
