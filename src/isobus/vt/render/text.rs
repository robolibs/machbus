//! Text measurement and simple text layout.
//!
//! The render layer does not ship a font rasteriser. This module models
//! text using a **monospace cell grid** derived from the resolved font
//! metrics (see [`FontMetrics`]). That is enough to:
//!
//! - compute whether a string fits a bounded output rectangle,
//! - clip / wrap a string to that rectangle,
//! - align it horizontally (left / middle / right) per the VT
//!   `justification` field,
//! - estimate the number of rows/columns occupied.
//!
//! Real glyph rendering is delegated to the host terminal; the GTUI
//! backend emits an opaque "draw text" command carrying the resolved
//! cell origin and the raw bytes.
//!
//! [`FontMetrics`]: crate::isobus::vt::render::style::FontMetrics

use crate::isobus::vt::render::style::FontMetrics;

/// Horizontal alignment derived from a VT `justification` field
/// (0 = left, 1 = middle, 2 = right).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HorizontalAlign {
    #[default]
    Left,
    Middle,
    Right,
}

/// Vertical alignment for richer text layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VerticalAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// One laid-out text row in device-pixel offsets relative to the node
/// rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLine {
    pub text: String,
    pub x_offset: i32,
    pub y_offset: i32,
}

/// Renderer-ready text layout. GTUI still emits simple text commands,
/// but richer backends can consume these row offsets for alignment,
/// wrapping, and clipping without re-measuring from scratch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLayout {
    pub lines: Vec<TextLine>,
    pub align: HorizontalAlign,
    pub vertical_align: VerticalAlign,
    pub clipped_cols: usize,
    pub clipped_rows: usize,
}

impl TextLayout {
    #[must_use]
    pub fn rendered(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

impl HorizontalAlign {
    #[must_use]
    pub const fn from_justification(v: u8) -> Self {
        match v {
            1 => Self::Middle,
            2 => Self::Right,
            _ => Self::Left,
        }
    }
}

/// A measured run of text: how many monospace columns and rows it
/// occupies for a given font, and what survives clipping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMeasure {
    /// Visible columns after clipping to the requested box.
    pub visible_cols: usize,
    /// Visible rows after wrapping/clipping to the requested box.
    pub visible_rows: usize,
    /// The byte content that fits the box (truncated to `visible_cols`
    /// per row and `visible_rows` rows).
    pub rendered: String,
    /// Columns that were hidden because the run exceeded the box width.
    pub clipped_cols: usize,
    /// Rows that were hidden because the run exceeded the box height.
    pub clipped_rows: usize,
}

impl TextMeasure {
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.visible_cols == 0 && self.visible_rows == 0
    }
}

/// Measure a string against a bounding box and font.
///
/// `box_w` / `box_h` are in device pixels. A zero dimension means "no
/// clip on that axis". The text is never wrapped mid-word: it is broken
/// on existing newlines and on overflow it is hard-clipped per row.
#[must_use]
pub fn measure(text: &str, metrics: FontMetrics, box_w: u16, box_h: u16) -> TextMeasure {
    let cell_w = metrics.cell_w.max(1);
    let cell_h = metrics.cell_h.max(1);

    let max_cols = if box_w == 0 {
        usize::MAX
    } else {
        (usize::from(box_w / cell_w)).max(1)
    };
    let max_rows = if box_h == 0 {
        usize::MAX
    } else {
        (usize::from(box_h / cell_h)).max(1)
    };

    let normalized = normalize_line_breaks(text);
    let raw_lines: Vec<&str> = if normalized.is_empty() {
        Vec::new()
    } else {
        normalized.split('\n').collect()
    };

    let mut rendered_rows: Vec<String> = Vec::new();
    let mut total_cols = 0usize;
    for line in &raw_lines {
        let chars: Vec<char> = line.chars().filter(|c| !is_combining(*c)).collect();
        total_cols = total_cols.max(chars.len());
        if rendered_rows.len() >= max_rows {
            continue;
        }
        if chars.len() <= max_cols {
            rendered_rows.push(chars.into_iter().collect());
        } else {
            rendered_rows.push(chars.into_iter().take(max_cols).collect());
        }
    }

    let visible_rows = rendered_rows.len();
    let visible_cols = rendered_rows
        .iter()
        .map(|r| r.chars().count())
        .max()
        .unwrap_or(0);
    let clipped_rows = raw_lines.len().saturating_sub(visible_rows);
    let clipped_cols = total_cols.saturating_sub(visible_cols);

    TextMeasure {
        visible_cols,
        visible_rows,
        rendered: rendered_rows.join("\n"),
        clipped_cols,
        clipped_rows,
    }
}

/// Build a renderer-ready text layout for a bounded rectangle.
///
/// `wrap=true` hard-wraps long rows at the available cell count; otherwise
/// long rows are clipped to the first visible cells. CRLF and CR line endings
/// are normalised to LF before measuring.
#[must_use]
pub fn layout_text(
    text: &str,
    metrics: FontMetrics,
    box_w: u16,
    box_h: u16,
    align: HorizontalAlign,
    vertical_align: VerticalAlign,
    wrap: bool,
) -> TextLayout {
    let cell_w = metrics.cell_w.max(1);
    let cell_h = metrics.cell_h.max(1);
    let max_cols = if box_w == 0 {
        usize::MAX
    } else {
        (usize::from(box_w / cell_w)).max(1)
    };
    let max_rows = if box_h == 0 {
        usize::MAX
    } else {
        (usize::from(box_h / cell_h)).max(1)
    };
    let normalized = normalize_line_breaks(text);
    let raw_lines: Vec<&str> = if normalized.is_empty() {
        Vec::new()
    } else {
        normalized.split('\n').collect()
    };
    let mut rows = Vec::new();
    let mut clipped_cols = 0usize;
    for raw in raw_lines {
        let chars: Vec<char> = raw.chars().filter(|c| !is_combining(*c)).collect();
        if wrap && max_cols != usize::MAX {
            if chars.is_empty() {
                rows.push(String::new());
            } else {
                for chunk in chars.chunks(max_cols) {
                    rows.push(chunk.iter().collect());
                }
            }
        } else if chars.len() <= max_cols {
            rows.push(chars.into_iter().collect());
        } else {
            clipped_cols = clipped_cols.max(chars.len().saturating_sub(max_cols));
            rows.push(chars.into_iter().take(max_cols).collect());
        }
    }

    let clipped_rows = rows.len().saturating_sub(max_rows);
    rows.truncate(max_rows);
    let visible_rows = rows.len();
    let content_h = i32::from(cell_h) * visible_rows as i32;
    let box_h_i = i32::from(box_h);
    let y_base = match vertical_align {
        VerticalAlign::Top => 0,
        VerticalAlign::Middle => ((box_h_i - content_h) / 2).max(0),
        VerticalAlign::Bottom => (box_h_i - content_h).max(0),
    };
    let lines = rows
        .into_iter()
        .enumerate()
        .map(|(row, text)| {
            let cols = text.chars().count();
            TextLine {
                text,
                x_offset: aligned_origin_x(align, cols, metrics, box_w),
                y_offset: y_base + i32::from(cell_h) * row as i32,
            }
        })
        .collect();

    TextLayout {
        lines,
        align,
        vertical_align,
        clipped_cols,
        clipped_rows,
    }
}

/// Compute the device-pixel x origin of a horizontally-aligned run
/// inside a box of width `box_w`.
#[must_use]
pub fn aligned_origin_x(
    align: HorizontalAlign,
    text_cols: usize,
    metrics: FontMetrics,
    box_w: u16,
) -> i32 {
    let cell_w = i32::from(metrics.cell_w.max(1));
    let box_w = i32::from(box_w);
    let text_w = cell_w * text_cols as i32;
    match align {
        HorizontalAlign::Left => 0,
        HorizontalAlign::Middle => ((box_w - text_w) / 2).max(0),
        HorizontalAlign::Right => (box_w - text_w).max(0),
    }
}

/// Return true when a VT string buffer starts with the ISO 11783-6
/// WideString marker (`FEFF16`, encoded on the wire as little-endian
/// `FF FE`).
#[must_use]
pub fn is_wide_string(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xFE])
}

/// Decode a VT string buffer (`Vec<u8>` body value) into a lossy Rust
/// string. ISO 11783-6 distinguishes 8-bit strings from UTF-16LE
/// WideStrings by a leading `FF FE` byte-order mark. We never panic on
/// malformed bytes and substitute `?` for invalid/unprintable content so the
/// renderer always has something deterministic to draw.
#[must_use]
pub fn decode_lossy(bytes: &[u8]) -> String {
    if is_wide_string(bytes) {
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]));
        return char::decode_utf16(units)
            .map(|item| item.unwrap_or('?'))
            .map(sanitize_decoded_char)
            .collect();
    }

    bytes
        .iter()
        .map(|&b| {
            // Printable ASCII, common whitespace, and Latin-1 letters pass
            // through unchanged. Everything else becomes '?' so the
            // renderer always has *something* to draw and never panics.
            let passthrough = (b.is_ascii() && !b.is_ascii_control())
                || matches!(b, b'\n' | b'\r' | b'\t')
                || (0xA0..=0xFF).contains(&b);
            if passthrough { char::from(b) } else { '?' }
        })
        .collect()
}

fn sanitize_decoded_char(c: char) -> char {
    if c.is_control() && !matches!(c, '\n' | '\r' | '\t') {
        '?'
    } else {
        c
    }
}

fn normalize_line_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    let _ = chars.next();
                }
                out.push('\n');
            }
            _ => out.push(c),
        }
    }
    out
}

#[must_use]
const fn is_combining(c: char) -> bool {
    // Minimal combining-mark guard: U+0300..U+036F covers the vast
    // majority of combining diacritics seen in Western object pools.
    let code = c as u32;
    code >= 0x0300 && code <= 0x036F
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> FontMetrics {
        FontMetrics {
            cell_w: 8,
            cell_h: 12,
            ascent: 10,
            descent: 2,
        }
    }

    #[test]
    fn empty_string_measures_to_zero_visible() {
        let m = measure("", metrics(), 80, 24);
        assert_eq!(m.visible_cols, 0);
        assert_eq!(m.visible_rows, 0);
        assert!(m.rendered.is_empty());
    }

    #[test]
    fn short_string_fits_without_clipping() {
        let m = measure("HELLO", metrics(), 80, 24);
        assert_eq!(m.visible_cols, 5);
        assert_eq!(m.visible_rows, 1);
        assert_eq!(m.rendered, "HELLO");
        assert_eq!(m.clipped_cols, 0);
    }

    #[test]
    fn overflow_is_clipped_per_row() {
        // box of 8px width / 8px cell -> 1 column; 5 chars -> clipped to 1.
        let m = measure("HELLO", metrics(), 8, 12);
        assert_eq!(m.visible_cols, 1);
        assert_eq!(m.clipped_cols, 4);
        assert_eq!(m.rendered, "H");
    }

    #[test]
    fn multiple_rows_clamped_to_box_height() {
        let m = measure("a\r\nb\rc\nd\ne", metrics(), 80, 24); // 24/12 = 2 rows
        assert_eq!(m.visible_rows, 2);
        assert_eq!(m.clipped_rows, 3);
    }

    #[test]
    fn layout_text_wraps_aligns_and_clips_rows() {
        let layout = layout_text(
            "ABCDE",
            metrics(),
            16,
            24,
            HorizontalAlign::Right,
            VerticalAlign::Bottom,
            true,
        );
        assert_eq!(layout.lines.len(), 2);
        assert_eq!(layout.lines[0].text, "AB");
        assert_eq!(layout.lines[1].text, "CD");
        assert_eq!(layout.clipped_rows, 1);
        assert_eq!(layout.lines[0].x_offset, 0);
        assert_eq!(layout.lines[0].y_offset, 0);
    }

    #[test]
    fn layout_text_normalises_crlf_and_tracks_offsets() {
        let layout = layout_text(
            "A\r\nB",
            metrics(),
            80,
            48,
            HorizontalAlign::Middle,
            VerticalAlign::Middle,
            false,
        );
        assert_eq!(layout.rendered(), "A\nB");
        assert_eq!(layout.lines[0].x_offset, 36);
        assert_eq!(layout.lines[0].y_offset, 12);
        assert_eq!(layout.lines[1].y_offset, 24);
    }

    #[test]
    fn aligned_origin_left_middle_right() {
        let mt = metrics();
        // 3 cols, 8px each = 24px; box 80px.
        assert_eq!(aligned_origin_x(HorizontalAlign::Left, 3, mt, 80), 0);
        assert_eq!(aligned_origin_x(HorizontalAlign::Middle, 3, mt, 80), 28);
        assert_eq!(aligned_origin_x(HorizontalAlign::Right, 3, mt, 80), 56);
    }

    #[test]
    fn decode_lossy_substitutes_invalid_bytes() {
        assert_eq!(decode_lossy(b"AB"), "AB");
        let s = decode_lossy(&[0x41, 0x01, 0x42]);
        assert_eq!(s, "A?B");
        // Latin-1 range passes through.
        assert_eq!(decode_lossy(&[0xE9]), "é");
    }

    #[test]
    fn decode_lossy_handles_iso_wide_strings() {
        assert!(is_wide_string(&[0xFF, 0xFE, 0x41, 0x00]));
        assert_eq!(
            decode_lossy(&[
                0xFF, 0xFE, // BOM
                0x41, 0x00, // A
                0xAC, 0x20, // €
                0x3D, 0xD8, 0x00, 0xDE, // 😀 surrogate pair
                0x99, // odd trailing byte is ignored per ISO 11783-6
            ]),
            "A€😀"
        );
        assert_eq!(
            decode_lossy(&[0xFF, 0xFE, 0x00, 0xD8]),
            "?",
            "malformed UTF-16 is rendered deterministically"
        );
        assert_eq!(
            decode_lossy(&[
                0xFF, 0xFE, // BOM
                0x41, 0x00, // A
                0x01, 0x00, // non-printing control
                0x0A, 0x00, // newline remains meaningful to layout
                0x42, 0x00, // B
            ]),
            "A?\nB",
            "WideString controls follow the same deterministic text policy as 8-bit strings"
        );
    }

    #[test]
    fn justification_maps_to_align() {
        assert_eq!(
            HorizontalAlign::from_justification(0),
            HorizontalAlign::Left
        );
        assert_eq!(
            HorizontalAlign::from_justification(1),
            HorizontalAlign::Middle
        );
        assert_eq!(
            HorizontalAlign::from_justification(2),
            HorizontalAlign::Right
        );
        assert_eq!(
            HorizontalAlign::from_justification(9),
            HorizontalAlign::Left
        );
    }
}
