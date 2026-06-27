//! Style model: colour palette, font metrics, resolved styles.
//!
//! The VT render layer does not own a GPU or a font atlas. Instead it
//! resolves the coarse-grained style attributes carried by VT objects
//! (font attributes, line attributes, fill attributes, raw colour
//! indices) into a concrete, renderer-ready [`ResolvedStyle`].
//!
//! ## Colour palette
//!
//! ISO 11783-6 object pools reference colours by an 8-bit index into a
//! terminal palette. The exact RGB values of the default palette are
//! standard material and are intentionally **not** copied into this
//! repository (see `GAP.md` — non-disclosure boundary).
//!
//! [`Palette::default_isobus`] therefore exposes a repo-owned,
//! deterministic approximation of the coarse palette structure that is
//! sufficient for layout, contrast checks, and offline rendering. Real
//! deployments override it with their terminal's calibrated palette via
//! [`Palette::set_entry`].
//!
//! [`ResolvedStyle`]: crate::isobus::vt::render::style::ResolvedStyle

use crate::isobus::vt::{
    FillAttributesBody, FontAttributesBody, LineAttributesBody, ObjectID, ObjectPool, ObjectType,
};

/// 24-bit sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Colour {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Colour {
    #[inline]
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[inline]
    #[must_use]
    pub const fn gray(v: u8) -> Self {
        Self::rgb(v, v, v)
    }

    /// Convert to the `[r, g, b]` triple used by the GTUI command list.
    #[inline]
    #[must_use]
    pub const fn to_array(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }
}

/// A 256-entry VT colour palette.
///
/// See the module docs for the non-disclosure rationale behind the
/// default approximation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    entries: [Colour; 256],
}

impl Default for Palette {
    fn default() -> Self {
        Self::default_isobus()
    }
}

impl Palette {
    /// Repo-owned deterministic approximation of the VT default palette.
    ///
    /// The structure follows the publicly known coarse regions of a VT
    /// palette (a low-index grey ramp, a saturated primary/secondary
    /// block, and reserved high indices) without reproducing any
    /// standard-prose table. Index 0 is white and index 1 is black,
    /// matching the common terminal background/foreground convention.
    #[must_use]
    pub fn default_isobus() -> Self {
        let mut entries = [Colour::rgb(0, 0, 0); 256];
        // Low indices: a neutral grey ramp from white (0) to black (15).
        entries[0] = Colour::rgb(255, 255, 255);
        for i in 1..16u16 {
            let v = (255 - (i as u32 * 17)).clamp(0, 255) as u8;
            entries[i as usize] = Colour::gray(v);
        }
        entries[1] = Colour::rgb(0, 0, 0);
        // 16..64: programmatically generated primaries/secondaries so the
        // renderer has visible contrast without copying standard values.
        for i in 16..64u16 {
            let r = (((i * 37) % 256) as u8).max(40);
            let g = (((i * 73) % 256) as u8).max(40);
            let b = (((i * 109) % 256) as u8).max(40);
            entries[i as usize] = Colour::rgb(r, g, b);
        }
        // 64..255: a broad, deterministic colour space.
        for i in 64..256u16 {
            let r = ((i * 53) % 256) as u8;
            let g = ((i * 97) % 256) as u8;
            let b = ((i * 151) % 256) as u8;
            entries[i as usize] = Colour::rgb(r, g, b);
        }
        // Reserved high indices stay black; keep the last entry a clear
        // "error magenta" so unresolved/invalid colours are visible.
        entries[255] = Colour::rgb(255, 0, 255);
        Self { entries }
    }

    /// Resolve an 8-bit colour index. Out-of-range indices (none exist
    /// for `u8`) always resolve to a defined entry.
    #[inline]
    #[must_use]
    pub fn resolve(&self, index: u8) -> Colour {
        self.entries[index as usize]
    }

    /// Replace a palette entry with a calibrated value.
    pub fn set_entry(&mut self, index: u8, colour: Colour) {
        self.entries[index as usize] = colour;
    }

    /// Overlay a VT6 Colour Palette object's ARGB entries, which override
    /// the default palette from index 0 upward (ISO 11783-6 §4.6.13). Each
    /// entry is a 32-bit `0xAARRGGBB` value; alpha is ignored by the current
    /// opaque RGB renderer and entries past index 255 are ignored.
    pub fn apply_colour_palette(&mut self, entries_argb: &[u32]) {
        for (index, argb) in entries_argb.iter().take(256).enumerate() {
            let r = ((argb >> 16) & 0xFF) as u8;
            let g = ((argb >> 8) & 0xFF) as u8;
            let b = (argb & 0xFF) as u8;
            self.entries[index] = Colour::rgb(r, g, b);
        }
    }

    #[inline]
    #[must_use]
    pub fn entries(&self) -> &[Colour; 256] {
        &self.entries
    }
}

/// Font weight / decoration carried by the VT `FontAttributes` style bit
/// set, decoded into a friendlier enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FontDecoration {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub inverted: bool,
}

impl FontDecoration {
    /// Decode the VT `FontAttributes.style` byte (bit 0 = bold, …).
    /// Reserved bits are ignored rather than rejected, because the
    /// renderer must never panic on a stylistically odd object pool.
    #[must_use]
    pub const fn from_style_byte(style: u8) -> Self {
        Self {
            bold: style & 0x01 != 0,
            italic: style & 0x02 != 0,
            underline: style & 0x04 != 0,
            strikethrough: style & 0x08 != 0,
            inverted: style & 0x10 != 0,
        }
    }
}

/// Resolved font metrics in device pixels.
///
/// The VT standard sizes fonts by a small enumeration. This struct maps
/// that enumeration to a concrete monospace cell size so the layout
/// engine and the GTUI renderer share one measurement source. The exact
/// glyph rasterisation is delegated to the host terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontMetrics {
    pub cell_w: u16,
    pub cell_h: u16,
    pub ascent: u16,
    pub descent: u16,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self::for_size(6)
    }
}

impl FontMetrics {
    /// Map a VT `font_size` enumeration (0..=14) to a monospace cell.
    ///
    /// The mapping is repo-owned and monotonic; it intentionally does
    /// not reproduce any standard-prose glyph table. Sizes outside the
    /// defined range clamp to the nearest bound instead of failing.
    #[must_use]
    pub fn for_size(size: u8) -> Self {
        let s = size.min(14);
        // Baseline 6x12 at size 0, growing roughly linearly to 16x28.
        let cell_h = 12u16 + (s as u16) * (28u16 - 12u16) / 14;
        let cell_w = 6u16 + (s as u16) * (16u16 - 6u16) / 14;
        let ascent = cell_h * 4 / 5;
        let descent = cell_h - ascent;
        Self {
            cell_w: cell_w.max(4),
            cell_h: cell_h.max(6),
            ascent,
            descent,
        }
    }

    #[inline]
    #[must_use]
    pub const fn line_height(self) -> u16 {
        self.cell_h
    }
}

/// A fully resolved visual style attached to a scene node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedStyle {
    pub foreground: Colour,
    pub background: Colour,
    pub font: FontMetrics,
    pub decoration: FontDecoration,
    pub line_width: u16,
    pub line_art: u16,
    pub fill_type: FillType,
    pub fill_colour: Colour,
}

impl Default for ResolvedStyle {
    fn default() -> Self {
        Self {
            foreground: Colour::rgb(0, 0, 0),
            background: Colour::rgb(255, 255, 255),
            font: FontMetrics::default(),
            decoration: FontDecoration::default(),
            line_width: 1,
            line_art: 0xFFFF,
            fill_type: FillType::None,
            fill_colour: Colour::rgb(255, 255, 255),
        }
    }
}

/// Decoded VT `FillAttributes.fill_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FillType {
    #[default]
    None,
    LineColour,
    FillColour,
    Pattern,
}

impl FillType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::LineColour,
            2 => Self::FillColour,
            3 => Self::Pattern,
            _ => Self::None,
        }
    }

    #[must_use]
    pub const fn is_solid(self) -> bool {
        matches!(self, Self::LineColour | Self::FillColour)
    }
}

/// Helper that resolves the style-relevant VT bodies against a pool and
/// palette. Created once per scene build and reused for every node.
#[derive(Debug, Clone)]
pub struct StyleResolver<'a> {
    pool: &'a ObjectPool,
    palette: Palette,
}

impl<'a> StyleResolver<'a> {
    #[must_use]
    pub fn new(pool: &'a ObjectPool, palette: Palette) -> Self {
        Self { pool, palette }
    }

    #[inline]
    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    #[inline]
    #[must_use]
    pub fn colour(&self, index: u8) -> Colour {
        self.palette.resolve(index)
    }

    /// Resolve a `FontAttributes` object reference into a concrete style.
    /// A `NULL` reference yields the default style.
    #[must_use]
    pub fn resolve_font(&self, font_ref: ObjectID) -> ResolvedStyle {
        let mut style = ResolvedStyle::default();
        if font_ref == ObjectID::NULL {
            return style;
        }
        let Some(obj) = self.pool.find(font_ref) else {
            return style;
        };
        if obj.r#type != ObjectType::FontAttributes {
            return style;
        }
        if let Ok(body) = obj.get_font_attributes_body() {
            style.foreground = self.palette.resolve(body.font_color);
            style.font = FontMetrics::for_size(body.font_size);
            style.decoration = FontDecoration::from_style_byte(body.font_style);
        }
        style
    }

    /// Overlay line-attribute information onto an existing style.
    #[must_use]
    pub fn overlay_line(&self, style: ResolvedStyle, line_ref: ObjectID) -> ResolvedStyle {
        if line_ref == ObjectID::NULL {
            return style;
        }
        let Some(obj) = self.pool.find(line_ref) else {
            return style;
        };
        if obj.r#type != ObjectType::LineAttributes {
            return style;
        }
        if let Ok(LineAttributesBody {
            line_color,
            line_width,
            line_art,
        }) = obj.get_line_attributes_body()
        {
            let mut s = style;
            s.foreground = self.palette.resolve(line_color);
            s.line_width = u16::from(line_width);
            s.line_art = line_art;
            return s;
        }
        style
    }

    /// Overlay fill-attribute information onto an existing style.
    #[must_use]
    pub fn overlay_fill(&self, style: ResolvedStyle, fill_ref: ObjectID) -> ResolvedStyle {
        if fill_ref == ObjectID::NULL {
            return style;
        }
        let Some(obj) = self.pool.find(fill_ref) else {
            return style;
        };
        if obj.r#type != ObjectType::FillAttributes {
            return style;
        }
        if let Ok(FillAttributesBody {
            fill_type,
            fill_color,
            ..
        }) = obj.get_fill_attributes_body()
        {
            let mut s = style;
            s.fill_type = FillType::from_u8(fill_type);
            s.fill_colour = match s.fill_type {
                FillType::LineColour => s.foreground,
                FillType::FillColour | FillType::Pattern | FillType::None => {
                    self.palette.resolve(fill_color)
                }
            };
            return s;
        }
        style
    }

    /// Decode a font-attributes body directly (used by the coverage
    /// ledger / inspector). Returns `None` if the object is missing or
    /// has the wrong type rather than propagating a codec error.
    #[must_use]
    pub fn font_body(&self, font_ref: ObjectID) -> Option<FontAttributesBody> {
        let obj = self.pool.find(font_ref)?;
        if obj.r#type != ObjectType::FontAttributes {
            return None;
        }
        obj.get_font_attributes_body().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_resolves_every_index_without_panicking() {
        let p = Palette::default_isobus();
        for i in 0u16..=255 {
            let _ = p.resolve(i as u8);
        }
        assert_eq!(p.resolve(0), Colour::rgb(255, 255, 255));
        assert_eq!(p.resolve(1), Colour::rgb(0, 0, 0));
        // The error sentinel must be visibly non-black.
        assert_ne!(p.resolve(255), Colour::rgb(0, 0, 0));
    }

    #[test]
    fn palette_set_entry_overrides_value() {
        let mut p = Palette::default_isobus();
        p.set_entry(10, Colour::rgb(1, 2, 3));
        assert_eq!(p.resolve(10), Colour::rgb(1, 2, 3));
    }

    #[test]
    fn apply_colour_palette_overrides_from_index_zero() {
        let mut p = Palette::default_isobus();
        p.apply_colour_palette(&[0x00_11_22_33, 0x00_AA_BB_CC]);
        assert_eq!(p.resolve(0), Colour::rgb(0x11, 0x22, 0x33));
        assert_eq!(p.resolve(1), Colour::rgb(0xAA, 0xBB, 0xCC));
        // Entries past the supplied list are untouched.
        assert_eq!(p.resolve(255), Colour::rgb(255, 0, 255));
    }

    #[test]
    fn font_metrics_is_monotonic_and_bounded() {
        let small = FontMetrics::for_size(0);
        let large = FontMetrics::for_size(14);
        assert!(large.cell_w >= small.cell_w);
        assert!(large.cell_h >= small.cell_h);
        // Out-of-range clamps rather than overflowing.
        let huge = FontMetrics::for_size(200);
        assert_eq!(huge.cell_w, large.cell_w);
        assert_eq!(huge.cell_h, large.cell_h);
    }

    #[test]
    fn fill_type_decodes_known_values_and_ignores_reserved() {
        assert_eq!(FillType::from_u8(0), FillType::None);
        assert_eq!(FillType::from_u8(2), FillType::FillColour);
        assert_eq!(FillType::from_u8(9), FillType::None);
    }

    #[test]
    fn font_decoration_decodes_style_bits() {
        let d = FontDecoration::from_style_byte(0x07);
        assert!(d.bold && d.italic && d.underline);
        assert!(!d.strikethrough && !d.inverted);
    }
}
