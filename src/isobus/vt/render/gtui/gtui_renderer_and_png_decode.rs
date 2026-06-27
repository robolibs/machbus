use crate::isobus::vt::render::scene::{
    FillPattern, NodeKind, Rect, Scene, SceneNode, SoftKeyKind, SoftKeyNode, UnsupportedRecord,
};
use crate::isobus::vt::render::style::{Colour, FillType, FontDecoration, Palette, ResolvedStyle};
use crate::isobus::vt::render::text::{
    self as text_layout, HorizontalAlign, TextLayout, VerticalAlign,
};
use crate::isobus::vt::{ObjectID, ObjectType};

const PICTURE_GRAPHIC_RLE: u8 = 0x04;
const GRAPHIC_DATA_RLE: u8 = 0x01;
const MAX_DECOMPRESSED_BITMAP_BYTES: usize = 8 * 1024 * 1024;
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1A\n";

fn vertical_align_from_justification(justification: u8) -> VerticalAlign {
    match (justification >> 2) & 0x03 {
        1 => VerticalAlign::Middle,
        2 => VerticalAlign::Bottom,
        _ => VerticalAlign::Top,
    }
}

/// Source region selected by a Graphics Context copy-to-picture command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsContextCopySource {
    /// Copy the full backing canvas.
    Canvas,
    /// Copy the current panned/zoomed viewport.
    Viewport,
}

/// One drawable command in the GTUI command list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderCommand {
    /// Fill a rectangle with a solid colour.
    FillRect { rect: Rect, colour: Colour },
    /// Stroke a rectangle outline (per-edge suppression bitmask as in
    /// the VT Output Rectangle `line_suppression` field).
    StrokeRect {
        rect: Rect,
        colour: Colour,
        width: u16,
        line_art: u16,
        suppress: u8,
    },
    /// Draw a single line (used by Output Line).
    Line {
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        colour: Colour,
        width: u16,
        line_art: u16,
    },
    /// Stroke / fill an ellipse.
    Ellipse {
        rect: Rect,
        colour: Colour,
        fill_colour: Colour,
        filled: bool,
        width: u16,
        line_art: u16,
    },
    /// Stroke / fill an ellipse arc/segment/section. Angles are carried in
    /// the VT object's raw half-degree units.
    EllipseArc {
        rect: Rect,
        colour: Colour,
        fill_colour: Colour,
        filled: bool,
        width: u16,
        line_art: u16,
        ellipse_type: u8,
        start_angle: u8,
        end_angle: u8,
    },
    /// Stroke / fill a polygon.
    Polygon {
        origin: (i32, i32),
        points: Vec<(i32, i32)>,
        colour: Colour,
        fill_colour: Colour,
        filled: bool,
        width: u16,
        line_art: u16,
    },
    /// Fill a rectangle by tiling a Picture Graphic pattern.
    PatternFillRect {
        rect: Rect,
        anchor: (i32, i32),
        pattern: FillPattern,
    },
    /// Fill a closed ellipse or ellipse section by tiling a Picture Graphic pattern.
    PatternFillEllipse {
        rect: Rect,
        anchor: (i32, i32),
        ellipse_type: u8,
        start_angle: u8,
        end_angle: u8,
        pattern: FillPattern,
    },
    /// Fill a closed polygon by tiling a Picture Graphic pattern.
    PatternFillPolygon {
        origin: (i32, i32),
        points: Vec<(i32, i32)>,
        anchor: (i32, i32),
        pattern: FillPattern,
    },
    /// Draw measured text inside a box with horizontal alignment.
    DrawText {
        rect: Rect,
        text: String,
        style: ResolvedStyle,
        align: HorizontalAlign,
        layout: TextLayout,
    },
    /// Draw a meter needle / arc placeholder.
    Meter {
        rect: Rect,
        value: u32,
        min: i32,
        max: i32,
        needle_colour: Colour,
        border_colour: Colour,
        arc_colour: Colour,
        show_value: bool,
        number_of_ticks: u8,
        start_angle: u8,
        end_angle: u8,
    },
    /// Draw a linear / arched bar graph.
    BarGraph {
        rect: Rect,
        value: u32,
        target_value: u32,
        min: i32,
        max: i32,
        arched: bool,
        colour: Colour,
        target_line_colour: Colour,
        show_border: bool,
        show_target_line: bool,
        show_ticks: bool,
        number_of_ticks: u8,
        line_only: bool,
        horizontal: bool,
        direction_positive: bool,
        clockwise: bool,
        start_angle: u8,
        end_angle: u8,
        bar_width: u16,
    },
    /// Draw an indexed bitmap payload. The host backend performs the
    /// final raster upload / pixel expansion using the current palette.
    IndexedImage {
        /// Source VT object that produced this image command.
        object_id: ObjectID,
        rect: Rect,
        width: u16,
        height: u16,
        format: u8,
        /// Whether `transparency` should be treated as a transparent index.
        transparent: bool,
        transparency: u8,
        data: Vec<u8>,
    },
    /// Draw a decoded true-colour PNG/RGBA image payload. Pixels are tightly
    /// packed RGBA8 in row-major order; alpha zero is transparent for
    /// framebuffer-style backends.
    RgbaImage {
        /// Source VT object that produced this image command.
        object_id: ObjectID,
        rect: Rect,
        width: u16,
        height: u16,
        data: Vec<u8>,
    },
    /// Update the visible viewport for a Graphics Context canvas.
    ///
    /// `zoom_raw` stores the wire-level 32-bit floating value so
    /// no_std/headless consumers can preserve exact command state without
    /// depending on host floating-point formatting.
    GraphicsContextViewport {
        object_id: ObjectID,
        viewport: Rect,
        zoom_raw: Option<u32>,
    },
    /// Copy Graphics Context pixels into a Picture Graphic object.
    ///
    /// This remains backend-neutral command intent. Backends can consume the
    /// replay/primitive stream plus this command directly; when the hosted
    /// runtime can replay the supported primitive/text-cell/Draw-VT-object
    /// subset into its bounded software canvas it also emits
    /// [`RenderCommand::GraphicsContextPictureData`].
    GraphicsContextCopyToPicture {
        object_id: ObjectID,
        picture_id: ObjectID,
        source: GraphicsContextCopySource,
        viewport: Rect,
        zoom_raw: Option<u32>,
    },
    /// Concrete indexed pixels copied from a Graphics Context canvas into a
    /// Picture Graphic target.
    ///
    /// This is emitted when the render runtime can replay the supported
    /// graphics-context primitive/text-cell/Draw-VT-object subset into its
    /// bounded software canvas.
    /// Backends that maintain their own canvas can still consume
    /// [`RenderCommand::GraphicsContextCopyToPicture`] as intent; headless and
    /// embedded backends can consume this payload directly.
    GraphicsContextPictureData {
        object_id: ObjectID,
        picture_id: ObjectID,
        source: GraphicsContextCopySource,
        width: u16,
        height: u16,
        format: u8,
        /// Canvas colour index that represents "do not copy" pixels for the
        /// originating Graphics Context copy operation.
        transparent_index: u8,
        data: Vec<u8>,
    },
    /// Declare the visible surface for a Graphic Context object.
    ///
    /// The command gives pixel/framebuffer backends the canvas dimensions and
    /// visible viewport rectangle before replay/drawing commands arrive. The
    /// `background` field is the VT colour index used for an opaque canvas'
    /// initial backing pixels and for replay state before an explicit
    /// Graphics Context background-colour subcommand changes it. When
    /// `transparent` is true, `transparency_colour` is the index used for
    /// untouched initial canvas pixels.
    GraphicsContextCanvas {
        object_id: ObjectID,
        rect: Rect,
        canvas_width: u16,
        canvas_height: u16,
        background: u8,
        transparency_colour: u8,
        transparent: bool,
    },
    /// Replay an accepted VT Graphics Context subcommand payload.
    ///
    /// This command intentionally preserves the validated wire-level
    /// subcommand data instead of rasterising it in the terminal renderer.
    /// Pixel backends can interpret the subcommand stream into a canvas,
    /// while tests and headless hosts still see deterministic draw-state
    /// changes.
    GraphicsContextReplay {
        object_id: ObjectID,
        subcommand: u8,
        payload: Vec<u8>,
    },
    /// Render a placeholder for an object the renderer cannot draw.
    Placeholder {
        rect: Rect,
        object_type: ObjectType,
        reason: String,
    },
    /// Render a soft-key cell.
    SoftKey {
        rect: Rect,
        kind: SoftKeyKind,
        label: String,
        latched: bool,
        style: ResolvedStyle,
    },
    /// Establish / restore a clip rectangle (push semantics handled by
    /// the host by position in the list).
    Clip(Rect),
}

/// The GTUI renderer. Pure: takes a scene + palette and produces a
/// command list.
#[derive(Debug, Clone)]
pub struct GtuiRenderer {
    palette: Palette,
}

impl Default for GtuiRenderer {
    fn default() -> Self {
        Self::new(Palette::default_isobus())
    }
}

impl GtuiRenderer {
    #[must_use]
    pub fn new(palette: Palette) -> Self {
        Self { palette }
    }

    #[inline]
    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    #[inline]
    #[must_use]
    fn palette_for_scene<'a>(&'a self, scene: &'a Scene) -> &'a Palette {
        scene.effective_palette.as_ref().unwrap_or(&self.palette)
    }

    /// Render a full scene to an ordered command list.
    #[must_use]
    pub fn render(&self, scene: &Scene) -> Vec<RenderCommand> {
        let mut out = Vec::new();
        let palette = self.palette_for_scene(scene);
        // 1) Mask background.
        let bg = palette.resolve(scene.background);
        out.push(RenderCommand::FillRect {
            rect: scene.mask_rect,
            colour: bg,
        });
        // Clip to the mask area so children never paint outside.
        out.push(RenderCommand::Clip(scene.mask_rect));

        // 2) Visible nodes.
        for node in scene.visible_nodes() {
            if let Some(clip) = node.clip {
                out.push(RenderCommand::Clip(clip));
            }
            let pattern_anchor = (scene.mask_rect.x, scene.mask_rect.y);
            self.emit_node(node, palette, pattern_anchor, bg, &mut out);
            if node.clip.is_some() {
                out.push(RenderCommand::Clip(scene.mask_rect));
            }
        }

        // 3) Soft-key column background + keys.
        if !scene.soft_keys.is_empty() {
            let sk_area = self.soft_key_bounds(scene);
            out.push(RenderCommand::Clip(sk_area));
            out.push(RenderCommand::FillRect {
                rect: sk_area,
                colour: palette.resolve(0),
            });
            for sk in &scene.soft_keys {
                if sk.visible {
                    self.emit_soft_key(sk, &mut out);
                }
            }
        }

        // 4) Unsupported markers as visible placeholders so an operator
        //    can see that something was rejected.
        for rec in &scene.unsupported {
            self.emit_unsupported_marker(rec, &mut out);
        }
        out
    }

    fn emit_node(
        &self,
        node: &SceneNode,
        palette: &Palette,
        pattern_anchor: (i32, i32),
        blank_background: Colour,
        out: &mut Vec<RenderCommand>,
    ) {
        match &node.kind {
            NodeKind::Group {
                background,
                transparent_bg,
                ..
            } => {
                if node.object_type == ObjectType::WindowMask && !node.enabled {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: blank_background,
                    });
                    return;
                }
                // Containers/masks draw their own background under their
                // children. We do not recurse here again because the
                // layout engine already flattened children into the
                // scene node list.
                if !transparent_bg {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: palette.resolve(*background),
                    });
                }
            }
            NodeKind::OutputString {
                text,
                transparent_bg,
                justification,
            } => {
                if !transparent_bg {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.background,
                    });
                }
                out.push(Self::draw_text_command(
                    node.rect,
                    text,
                    node.style,
                    HorizontalAlign::from_justification(*justification),
                    vertical_align_from_justification(*justification),
                    true,
                ));
            }
            NodeKind::OutputNumber {
                text,
                transparent_bg,
                justification,
            } => {
                if !transparent_bg {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.background,
                    });
                }
                out.push(Self::draw_text_command(
                    node.rect,
                    text,
                    node.style,
                    HorizontalAlign::from_justification(*justification & 0x03),
                    vertical_align_from_justification(*justification),
                    false,
                ));
            }
            NodeKind::OutputList {
                selected_text,
                selected_item_materialized,
                ..
            } => {
                if *selected_item_materialized {
                    return;
                }
                let text = selected_text.clone().unwrap_or_default();
                out.push(Self::draw_text_command(
                    node.rect,
                    &text,
                    node.style,
                    HorizontalAlign::Left,
                    VerticalAlign::Top,
                    false,
                ));
            }
            NodeKind::OutputLine { direction } => {
                if node.style.line_width != 0 {
                    let (x0, y0, x1, y1) = match direction {
                        1 => (
                            node.rect.right() - 1,
                            node.rect.y,
                            node.rect.x,
                            node.rect.bottom() - 1,
                        ),
                        _ => (
                            node.rect.x,
                            node.rect.y,
                            node.rect.right() - 1,
                            node.rect.bottom() - 1,
                        ),
                    };
                    out.push(RenderCommand::Line {
                        x0,
                        y0,
                        x1,
                        y1,
                        colour: node.style.foreground,
                        width: node.style.line_width,
                        line_art: node.style.line_art,
                    });
                }
            }
            NodeKind::OutputRectangle {
                line_suppression,
                fill_pattern,
            } => {
                if node.style.fill_type.is_solid() {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.fill_colour,
                    });
                } else if node.style.fill_type == FillType::Pattern
                    && let Some(pattern) = fill_pattern
                {
                    out.push(RenderCommand::PatternFillRect {
                        rect: node.rect,
                        anchor: pattern_anchor,
                        pattern: pattern.clone(),
                    });
                }
                if node.style.line_width != 0 {
                    out.push(RenderCommand::StrokeRect {
                        rect: node.rect,
                        colour: node.style.foreground,
                        width: node.style.line_width,
                        line_art: node.style.line_art,
                        suppress: *line_suppression,
                    });
                }
            }
            NodeKind::OutputEllipse {
                filled,
                fill_pattern,
                closed,
                ellipse_type,
                start_angle,
                end_angle,
            } => {
                if *closed {
                    if !*filled
                        && node.style.fill_type == FillType::Pattern
                        && let Some(pattern) = fill_pattern
                    {
                        out.push(RenderCommand::PatternFillEllipse {
                            rect: node.rect,
                            anchor: pattern_anchor,
                            ellipse_type: 0,
                            start_angle: *start_angle,
                            end_angle: *end_angle,
                            pattern: pattern.clone(),
                        });
                    }
                    if *filled || node.style.line_width != 0 {
                        out.push(RenderCommand::Ellipse {
                            rect: node.rect,
                            colour: node.style.foreground,
                            fill_colour: node.style.fill_colour,
                            filled: *filled,
                            width: node.style.line_width,
                            line_art: node.style.line_art,
                        });
                    }
                } else if *filled
                    || node.style.line_width != 0
                    || (node.style.fill_type == FillType::Pattern && fill_pattern.is_some())
                {
                    if !*filled
                        && node.style.fill_type == FillType::Pattern
                        && let Some(pattern) = fill_pattern
                    {
                        out.push(RenderCommand::PatternFillEllipse {
                            rect: node.rect,
                            anchor: pattern_anchor,
                            ellipse_type: *ellipse_type,
                            start_angle: *start_angle,
                            end_angle: *end_angle,
                            pattern: pattern.clone(),
                        });
                    }
                    out.push(RenderCommand::EllipseArc {
                        rect: node.rect,
                        colour: node.style.foreground,
                        fill_colour: node.style.fill_colour,
                        filled: *filled,
                        width: node.style.line_width,
                        line_art: node.style.line_art,
                        ellipse_type: *ellipse_type,
                        start_angle: *start_angle,
                        end_angle: *end_angle,
                    });
                }
            }
            NodeKind::OutputPolygon {
                points,
                fill_pattern,
            } => {
                let filled = node.style.fill_type.is_solid();
                let device_points = points
                    .iter()
                    .map(|&(x, y)| (i32::from(x) + node.rect.x, i32::from(y) + node.rect.y))
                    .collect::<Vec<_>>();
                if !filled
                    && node.style.fill_type == FillType::Pattern
                    && let Some(pattern) = fill_pattern
                {
                    out.push(RenderCommand::PatternFillPolygon {
                        origin: (node.rect.x, node.rect.y),
                        points: device_points.clone(),
                        anchor: pattern_anchor,
                        pattern: pattern.clone(),
                    });
                }
                if filled || node.style.line_width != 0 {
                    out.push(RenderCommand::Polygon {
                        origin: (node.rect.x, node.rect.y),
                        points: device_points,
                        colour: node.style.foreground,
                        fill_colour: node.style.fill_colour,
                        filled,
                        width: node.style.line_width,
                        line_art: node.style.line_art,
                    });
                }
            }
            NodeKind::Meter {
                value,
                min_value,
                max_value,
                needle_colour,
                border_colour,
                arc_colour,
                show_value,
                number_of_ticks,
                start_angle,
                end_angle,
            } => {
                out.push(RenderCommand::Meter {
                    rect: node.rect,
                    value: *value,
                    min: *min_value,
                    max: *max_value,
                    needle_colour: palette.resolve(*needle_colour),
                    border_colour: palette.resolve(*border_colour),
                    arc_colour: palette.resolve(*arc_colour),
                    show_value: *show_value,
                    number_of_ticks: *number_of_ticks,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                });
            }
            NodeKind::LinearBarGraph {
                value,
                target_value,
                min_value,
                max_value,
                colour,
                target_line_colour,
                show_border,
                show_target_line,
                show_ticks,
                number_of_ticks,
                line_only,
                horizontal,
                direction_positive,
            } => out.push(RenderCommand::BarGraph {
                rect: node.rect,
                value: *value,
                target_value: *target_value,
                min: *min_value,
                max: *max_value,
                arched: false,
                colour: palette.resolve(*colour),
                target_line_colour: palette.resolve(*target_line_colour),
                show_border: *show_border,
                show_target_line: *show_target_line,
                show_ticks: *show_ticks,
                number_of_ticks: *number_of_ticks,
                line_only: *line_only,
                horizontal: *horizontal,
                direction_positive: *direction_positive,
                clockwise: false,
                start_angle: 0,
                end_angle: 0,
                bar_width: 0,
            }),
            NodeKind::ArchedBarGraph {
                value,
                target_value,
                min_value,
                max_value,
                colour,
                target_line_colour,
                show_border,
                show_target_line,
                line_only,
                clockwise,
                start_angle,
                end_angle,
                bar_width,
            } => out.push(RenderCommand::BarGraph {
                rect: node.rect,
                value: *value,
                target_value: *target_value,
                min: *min_value,
                max: *max_value,
                arched: true,
                colour: palette.resolve(*colour),
                target_line_colour: palette.resolve(*target_line_colour),
                show_border: *show_border,
                show_target_line: *show_target_line,
                show_ticks: false,
                number_of_ticks: 0,
                line_only: *line_only,
                horizontal: false,
                direction_positive: true,
                clockwise: *clockwise,
                start_angle: *start_angle,
                end_angle: *end_angle,
                bar_width: *bar_width,
            }),
            NodeKind::PictureGraphic {
                raw_width,
                raw_height,
                format,
                options,
                transparency,
                data,
            } => {
                if options & PICTURE_GRAPHIC_RLE == 0 {
                    push_indexed_image_or_placeholder(
                        out,
                        node.id,
                        node.rect,
                        ObjectType::PictureGraphic,
                        "picture",
                        *raw_width,
                        *raw_height,
                        *format,
                        options & 0x01 != 0,
                        *transparency,
                        data.clone(),
                        ExtraBitmapData::Ignore,
                    );
                } else if let Some(decoded) =
                    decode_rle_pairs_for_indexed_bitmap(data, *raw_width, *raw_height, *format)
                {
                    push_indexed_image_or_placeholder(
                        out,
                        node.id,
                        node.rect,
                        ObjectType::PictureGraphic,
                        "picture",
                        *raw_width,
                        *raw_height,
                        *format,
                        options & 0x01 != 0,
                        *transparency,
                        decoded,
                        ExtraBitmapData::Ignore,
                    );
                } else {
                    out.push(RenderCommand::Placeholder {
                        rect: node.rect,
                        object_type: ObjectType::PictureGraphic,
                        reason: format!(
                            "picture {raw_width}x{raw_height} fmt={format} has invalid compressed payload"
                        ),
                    });
                }
            }
            NodeKind::ScaledGraphic {
                width,
                height,
                format,
                options,
                standard_png,
                transparent,
                transparency,
                data,
            } => {
                if *standard_png {
                    match decode_standard_png_graphic_data(data) {
                        Ok(decoded) => out.push(RenderCommand::RgbaImage {
                            object_id: node.id,
                            rect: node.rect,
                            width: decoded.width,
                            height: decoded.height,
                            data: decoded.data,
                        }),
                        Err(reason) => out.push(RenderCommand::Placeholder {
                            rect: node.rect,
                            object_type: ObjectType::ScaledGraphic,
                            reason: png_graphic_data_placeholder_reason(
                                "scaled graphic",
                                *width,
                                *height,
                                data,
                                reason,
                            ),
                        }),
                    }
                } else if options & GRAPHIC_DATA_RLE == 0 {
                    push_indexed_image_or_placeholder(
                        out,
                        node.id,
                        node.rect,
                        ObjectType::ScaledGraphic,
                        "scaled graphic",
                        *width,
                        *height,
                        *format,
                        *transparent,
                        *transparency,
                        data.clone(),
                        ExtraBitmapData::Ignore,
                    );
                } else if let Some(decoded) =
                    decode_rle_pairs_for_indexed_bitmap(data, *width, *height, *format)
                {
                    push_indexed_image_or_placeholder(
                        out,
                        node.id,
                        node.rect,
                        ObjectType::ScaledGraphic,
                        "scaled graphic",
                        *width,
                        *height,
                        *format,
                        *transparent,
                        *transparency,
                        decoded,
                        ExtraBitmapData::Ignore,
                    );
                } else {
                    out.push(RenderCommand::Placeholder {
                        rect: node.rect,
                        object_type: ObjectType::ScaledGraphic,
                        reason: format!(
                            "scaled graphic {width}x{height} fmt={format} has invalid compressed payload"
                        ),
                    });
                }
            }
            NodeKind::ScaledBitmap {
                width,
                height,
                format,
                options,
                data,
            } => {
                let payload = if options & GRAPHIC_DATA_RLE == 0 {
                    Some(data.clone())
                } else {
                    decode_rle_pairs(data)
                };
                match payload {
                    None => out.push(RenderCommand::Placeholder {
                        rect: node.rect,
                        object_type: ObjectType::ScaledBitmap,
                        reason: format!(
                            "scaled bitmap {width}x{height} fmt={format} has invalid compressed payload"
                        ),
                    }),
                    // Format 3 is direct 24-bit RGB rather than an indexed palette.
                    Some(bytes) if *format == 3 => push_rgb24_image_or_placeholder(
                        out,
                        node.id,
                        node.rect,
                        ObjectType::ScaledBitmap,
                        "scaled bitmap",
                        *width,
                        *height,
                        bytes,
                    ),
                    Some(bytes) => push_indexed_image_or_placeholder(
                        out,
                        node.id,
                        node.rect,
                        ObjectType::ScaledBitmap,
                        "scaled bitmap",
                        *width,
                        *height,
                        *format,
                        false,
                        u8::MAX,
                        bytes,
                        ExtraBitmapData::Reject,
                    ),
                }
            }
            NodeKind::GraphicContext {
                canvas_width,
                canvas_height,
                background,
                transparency_colour,
                transparent,
            } => {
                out.push(RenderCommand::GraphicsContextCanvas {
                    object_id: node.id,
                    rect: node.rect,
                    canvas_width: *canvas_width,
                    canvas_height: *canvas_height,
                    background: *background,
                    transparency_colour: *transparency_colour,
                    transparent: *transparent,
                });
                if !transparent {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: palette.resolve(*background),
                    });
                }
            }
            NodeKind::GraphicsContext {
                fill_rgb,
                line_rgb,
                line_width,
                line_style,
            } => {
                // machbus compatibility extension (type 50): the geometry-less
                // graphics-context state is drawn as a fill+border swatch using
                // its own 24-bit RGB state (R low byte, then G, then B).
                let rgb24 = |rgb: u32| {
                    Colour::rgb(
                        (rgb & 0xFF) as u8,
                        ((rgb >> 8) & 0xFF) as u8,
                        ((rgb >> 16) & 0xFF) as u8,
                    )
                };
                out.push(RenderCommand::FillRect {
                    rect: node.rect,
                    colour: rgb24(*fill_rgb),
                });
                if *line_width > 0 {
                    out.push(RenderCommand::StrokeRect {
                        rect: node.rect,
                        colour: rgb24(*line_rgb),
                        width: *line_width,
                        // Map the machbus line style (0 solid / 1 dashed /
                        // 2 dotted) to a representative 16-bit line-art pattern.
                        line_art: match line_style {
                            1 => 0xF0F0,
                            2 => 0xAAAA,
                            _ => 0xFFFF,
                        },
                        suppress: 0,
                    });
                }
            }
            NodeKind::InputBoolean { enabled, value } => {
                let bg = if *value {
                    node.style.foreground
                } else {
                    node.style.background
                };
                out.push(RenderCommand::FillRect {
                    rect: node.rect,
                    colour: bg,
                });
                out.push(RenderCommand::StrokeRect {
                    rect: node.rect,
                    colour: node.style.foreground,
                    width: node.style.line_width,
                    line_art: 0xFFFF,
                    suppress: 0,
                });
                if !enabled {
                    self.dim(node.rect, out);
                }
            }
            NodeKind::InputString {
                text,
                enabled,
                transparent_bg,
                auto_wrap,
                justification,
                ..
            } => {
                if !transparent_bg {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.background,
                    });
                }
                out.push(Self::draw_text_command(
                    node.rect,
                    text,
                    node.style,
                    HorizontalAlign::from_justification(*justification & 0x03),
                    vertical_align_from_justification(*justification),
                    *auto_wrap,
                ));
                out.push(RenderCommand::StrokeRect {
                    rect: node.rect,
                    colour: node.style.foreground,
                    width: node.style.line_width,
                    line_art: 0xFFFF,
                    suppress: 0,
                });
                if !enabled {
                    self.dim(node.rect, out);
                }
            }
            NodeKind::InputNumber {
                text,
                enabled,
                transparent_bg,
                justification,
                ..
            } => {
                if !transparent_bg {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.background,
                    });
                }
                out.push(Self::draw_text_command(
                    node.rect,
                    text,
                    node.style,
                    HorizontalAlign::from_justification(*justification & 0x03),
                    vertical_align_from_justification(*justification),
                    false,
                ));
                out.push(RenderCommand::StrokeRect {
                    rect: node.rect,
                    colour: node.style.foreground,
                    width: node.style.line_width,
                    line_art: 0xFFFF,
                    suppress: 0,
                });
                if !enabled {
                    self.dim(node.rect, out);
                }
            }
            NodeKind::InputList {
                enabled,
                selected_text,
                selected_item_materialized,
                ..
            } => {
                out.push(RenderCommand::StrokeRect {
                    rect: node.rect,
                    colour: node.style.foreground,
                    width: node.style.line_width,
                    line_art: 0xFFFF,
                    suppress: 0,
                });
                if !selected_item_materialized {
                    let text = selected_text.clone().unwrap_or_default();
                    out.push(Self::draw_text_command(
                        node.rect,
                        &text,
                        node.style,
                        HorizontalAlign::Left,
                        VerticalAlign::Top,
                        false,
                    ));
                }
                if !enabled {
                    self.dim(node.rect, out);
                }
            }
            NodeKind::Button {
                label,
                enabled,
                transparent_bg,
                draw_border,
                ..
            } => {
                if !transparent_bg {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.background,
                    });
                }
                if *draw_border {
                    out.push(RenderCommand::StrokeRect {
                        rect: node.rect,
                        colour: node.style.foreground,
                        width: node.style.line_width,
                        line_art: 0xFFFF,
                        suppress: 0,
                    });
                }
                if !label.is_empty() {
                    out.push(Self::draw_text_command(
                        node.rect,
                        label,
                        node.style,
                        HorizontalAlign::Middle,
                        VerticalAlign::Middle,
                        false,
                    ));
                }
                if !enabled {
                    self.dim(node.rect, out);
                }
            }
            NodeKind::KeyDesignator { label, .. } => {
                out.push(RenderCommand::SoftKey {
                    rect: node.rect,
                    kind: SoftKeyKind::Application,
                    label: label.clone(),
                    latched: false,
                    style: node.style,
                });
            }
            NodeKind::KeyGroup {
                available,
                transparent,
                key_ids,
                labels,
                ..
            } => {
                if !available || !transparent {
                    out.push(RenderCommand::FillRect {
                        rect: node.rect,
                        colour: node.style.background,
                    });
                }
                if *available {
                    let cells = labels.len().max(1);
                    let horizontal = node.rect.w > node.rect.h;
                    let cell_w = if horizontal {
                        (node.rect.w / cells as u16).max(1)
                    } else {
                        node.rect.w
                    };
                    let cell_h = if horizontal {
                        node.rect.h
                    } else {
                        (node.rect.h / cells as u16).max(1)
                    };
                    for (i, label) in labels.iter().enumerate() {
                        if key_ids.get(i).copied() == Some(ObjectID::NULL) {
                            continue;
                        }
                        out.push(RenderCommand::SoftKey {
                            rect: Rect::new(
                                node.rect.x
                                    + if horizontal {
                                        i as i32 * i32::from(cell_w)
                                    } else {
                                        0
                                    },
                                node.rect.y
                                    + if horizontal {
                                        0
                                    } else {
                                        i as i32 * i32::from(cell_h)
                                    },
                                cell_w,
                                cell_h,
                            ),
                            kind: SoftKeyKind::Application,
                            label: label.clone(),
                            latched: false,
                            style: node.style,
                        });
                    }
                }
            }
            NodeKind::Unsupported { type_byte, reason } => {
                out.push(RenderCommand::Placeholder {
                    rect: node.rect,
                    object_type: ObjectType::from_u8(*type_byte),
                    reason: (*reason).to_string(),
                });
            }
        }
    }

    fn emit_soft_key(&self, sk: &SoftKeyNode, out: &mut Vec<RenderCommand>) {
        out.push(RenderCommand::SoftKey {
            rect: sk.rect,
            kind: sk.kind,
            label: sk.label.clone(),
            latched: false,
            style: sk.style,
        });
    }

    fn draw_text_command(
        rect: Rect,
        raw_text: &str,
        style: ResolvedStyle,
        align: HorizontalAlign,
        vertical_align: VerticalAlign,
        wrap: bool,
    ) -> RenderCommand {
        let layout = text_layout::layout_text(
            raw_text,
            style.font,
            rect.w,
            rect.h,
            align,
            vertical_align,
            wrap,
        );
        RenderCommand::DrawText {
            rect,
            text: layout.rendered(),
            style,
            align,
            layout,
        }
    }

    fn emit_unsupported_marker(&self, rec: &UnsupportedRecord, out: &mut Vec<RenderCommand>) {
        out.push(RenderCommand::Placeholder {
            rect: Rect::new(0, 0, 0, 0),
            object_type: rec.object_type,
            reason: rec.reason.to_string(),
        });
    }

    /// Overlay a translucent "dimmed" marker on a disabled field. With
    /// no alpha available, we stroke a dashed-looking double border.
    fn dim(&self, rect: Rect, out: &mut Vec<RenderCommand>) {
        out.push(RenderCommand::StrokeRect {
            rect: Rect::new(
                rect.x + 2,
                rect.y + 2,
                rect.w.saturating_sub(4),
                rect.h.saturating_sub(4),
            ),
            colour: Colour::gray(128),
            width: 1,
            line_art: 0xAAAA,
            suppress: 0,
        });
    }

    fn soft_key_bounds(&self, scene: &Scene) -> Rect {
        scene
            .soft_keys
            .iter()
            .map(|s| s.rect)
            .fold(None::<Rect>, |acc, r| match acc {
                None => Some(r),
                Some(a) => {
                    let x = a.x.min(r.x);
                    let y = a.y.min(r.y);
                    let right = (a.right()).max(r.right());
                    let bottom = (a.bottom()).max(r.bottom());
                    let w = (right - x).max(0) as u16;
                    let h = (bottom - y).max(0) as u16;
                    Some(Rect::new(x, y, w, h))
                }
            })
            .unwrap_or(Rect::default())
    }

    /// Convenience: render just the visible-node count, for quick logs.
    #[must_use]
    pub fn count_drawables(&self, scene: &Scene) -> usize {
        scene
            .nodes
            .iter()
            .filter(|n| n.visible && !matches!(n.kind, NodeKind::Unsupported { .. }))
            .count()
    }
}

/// Public helper: build a [`ResolvedStyle`] with explicit colours. Used
/// by tests/examples so they don't poke private fields.
#[must_use]
pub fn solid_style(foreground: Colour, background: Colour) -> ResolvedStyle {
    ResolvedStyle {
        foreground,
        background,
        font: crate::isobus::vt::render::style::FontMetrics::default(),
        decoration: FontDecoration::default(),
        line_width: 1,
        line_art: 0xFFFF,
        fill_type: FillType::None,
        fill_colour: background,
    }
}

fn decode_rle_pairs(data: &[u8]) -> Option<Vec<u8>> {
    if !data.len().is_multiple_of(2) {
        return None;
    }
    let mut decoded_len = 0usize;
    for pair in data.chunks_exact(2) {
        decoded_len = decoded_len.checked_add(usize::from(pair[0]))?;
        if decoded_len > MAX_DECOMPRESSED_BITMAP_BYTES {
            return None;
        }
    }
    let mut out = Vec::with_capacity(decoded_len);
    for pair in data.chunks_exact(2) {
        let next_len = out.len().checked_add(usize::from(pair[0]))?;
        out.resize(next_len, pair[1]);
    }
    Some(out)
}

fn decode_rle_pairs_for_indexed_bitmap(
    data: &[u8],
    width: u16,
    height: u16,
    format: u8,
) -> Option<Vec<u8>> {
    let Some(required) = indexed_bitmap_required_bytes(width, height, format) else {
        return decode_rle_pairs(data);
    };
    decode_rle_pairs_up_to(data, required)
}

fn decode_rle_pairs_up_to(data: &[u8], required: usize) -> Option<Vec<u8>> {
    if !data.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(required);
    for pair in data.chunks_exact(2) {
        let count = usize::from(pair[0]);
        if count == 0 {
            continue;
        }
        let remaining = required.saturating_sub(out.len());
        let written = count.min(remaining);
        let next_len = out.len().checked_add(written)?;
        out.resize(next_len, pair[1]);
        if out.len() == required {
            break;
        }
    }
    Some(out)
}

fn indexed_bitmap_required_bytes(width: u16, height: u16, format: u8) -> Option<usize> {
    let width = usize::from(width);
    let height = usize::from(height);
    let row = match format {
        0 => width.saturating_add(7) / 8,
        1 => width.saturating_add(1) / 2,
        2 => width,
        _ => return None,
    };
    row.checked_mul(height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PngHeader {
    width: u32,
    height: u32,
    bit_depth: u8,
    colour_type: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PngRgbaImage {
    width: u16,
    height: u16,
    data: Vec<u8>,
}

fn png_graphic_data_placeholder_reason(
    label: &'static str,
    target_width: u16,
    target_height: u16,
    data: &[u8],
    decode_reason: &'static str,
) -> String {
    match inspect_standard_png_graphic_data(data) {
        Ok(header) => format!(
            "{label} {target_width}x{target_height} references standard PNG GraphicData {}x{} bit_depth={} colour_type={}; PNG decode unsupported: {decode_reason}",
            header.width, header.height, header.bit_depth, header.colour_type
        ),
        Err(reason) => format!(
            "{label} {target_width}x{target_height} references malformed standard PNG GraphicData: {reason}"
        ),
    }
}

fn decode_standard_png_graphic_data(data: &[u8]) -> Result<PngRgbaImage, &'static str> {
    let parsed = parse_standard_png_graphic_data(data)?;
    if parsed.header.width > u16::MAX as u32 || parsed.header.height > u16::MAX as u32 {
        return Err("PNG dimensions exceed hosted render command limits");
    }
    if !matches!(parsed.header.colour_type, 0 | 2 | 3 | 4 | 6) {
        return Err(
            "only grayscale, grayscale-alpha, indexed-colour, RGB, and RGBA PNGs are decoded",
        );
    }
    let bits_per_pixel = png_bits_per_pixel(parsed.header.bit_depth, parsed.header.colour_type)
        .ok_or("IHDR colour type/bit depth is invalid")?;
    if bits_per_pixel > 64 {
        return Err("PNG exceeds the 64-bit-per-pixel GraphicData limit");
    }
    // 8- and 16-bit RGB/RGBA are decoded; 16-bit channels are down-sampled to
    // 8 bits per channel into the backend-neutral RGBA8 command.
    let inflated = zlib_deflate_decompress(&parsed.idat)?;
    let width = u16::try_from(parsed.header.width).map_err(|_| "PNG width is too large")?;
    let height = u16::try_from(parsed.header.height).map_err(|_| "PNG height is too large")?;
    let pixel_count = usize::from(width)
        .checked_mul(usize::from(height))
        .ok_or("PNG pixel count overflows")?;
    let mut rgba = vec![
        0;
        pixel_count
            .checked_mul(4)
            .ok_or("PNG RGBA byte count overflows")?
    ];
    match parsed.interlace {
        0 => decode_png_non_interlaced(&parsed, &inflated, width, height, &mut rgba)?,
        1 => decode_png_adam7_interlaced(&parsed, &inflated, width, height, &mut rgba)?,
        _ => return Err("PNG interlace method is invalid"),
    }
    Ok(PngRgbaImage {
        width,
        height,
        data: rgba,
    })
}

fn decode_png_non_interlaced(
    parsed: &ParsedPngData,
    inflated: &[u8],
    width: u16,
    height: u16,
    rgba: &mut [u8],
) -> Result<(), &'static str> {
    let row_bytes = png_scanline_bytes(width, parsed.header.bit_depth, parsed.header.colour_type)
        .ok_or("PNG row byte count overflows")?;
    let bytes_per_pixel =
        png_filter_bytes_per_pixel(parsed.header.bit_depth, parsed.header.colour_type)
            .ok_or("PNG colour type/bit depth is not implemented")?;
    let expected = usize::from(height)
        .checked_mul(
            row_bytes
                .checked_add(1)
                .ok_or("PNG row byte count overflows")?,
        )
        .ok_or("PNG decoded byte count overflows")?;
    if inflated.len() != expected {
        return Err("PNG decoded scanline byte count does not match IHDR dimensions");
    }
    let mut previous = vec![0; row_bytes];
    let mut current = vec![0; row_bytes];
    for row in 0..usize::from(height) {
        let source = row
            .checked_mul(row_bytes + 1)
            .ok_or("PNG row offset overflows")?;
        let filter = inflated[source];
        current.copy_from_slice(&inflated[source + 1..source + 1 + row_bytes]);
        png_unfilter_row(filter, bytes_per_pixel, &previous, &mut current)?;
        let out_row = row
            .checked_mul(usize::from(width))
            .and_then(|value| value.checked_mul(4))
            .ok_or("PNG RGBA row offset overflows")?;
        decode_png_row_to_rgba(
            parsed,
            &current,
            usize::from(width),
            &mut rgba[out_row..out_row + usize::from(width) * 4],
        )?;
        previous.copy_from_slice(&current);
    }
    Ok(())
}

fn decode_png_adam7_interlaced(
    parsed: &ParsedPngData,
    inflated: &[u8],
    width: u16,
    height: u16,
    rgba: &mut [u8],
) -> Result<(), &'static str> {
    const ADAM7_PASSES: [(usize, usize, usize, usize); 7] = [
        (0, 0, 8, 8),
        (4, 0, 8, 8),
        (0, 4, 4, 8),
        (2, 0, 4, 4),
        (0, 2, 2, 4),
        (1, 0, 2, 2),
        (0, 1, 1, 2),
    ];
    let width = usize::from(width);
    let height = usize::from(height);
    let bytes_per_pixel =
        png_filter_bytes_per_pixel(parsed.header.bit_depth, parsed.header.colour_type)
            .ok_or("PNG colour type/bit depth is not implemented")?;
    let mut offset = 0usize;
    for (x_start, y_start, x_step, y_step) in ADAM7_PASSES {
        let pass_width = adam7_axis_count(width, x_start, x_step);
        let pass_height = adam7_axis_count(height, y_start, y_step);
        if pass_width == 0 || pass_height == 0 {
            continue;
        }
        let pass_width_u16 =
            u16::try_from(pass_width).map_err(|_| "PNG Adam7 pass width is too large")?;
        let row_bytes = png_scanline_bytes(
            pass_width_u16,
            parsed.header.bit_depth,
            parsed.header.colour_type,
        )
        .ok_or("PNG Adam7 row byte count overflows")?;
        let mut previous = vec![0; row_bytes];
        let mut current = vec![0; row_bytes];
        let mut row_rgba = vec![0; pass_width * 4];
        for pass_y in 0..pass_height {
            let row_end = offset
                .checked_add(1)
                .and_then(|value| value.checked_add(row_bytes))
                .ok_or("PNG Adam7 row offset overflows")?;
            if row_end > inflated.len() {
                return Err("PNG Adam7 decoded scanline byte count is truncated");
            }
            let filter = inflated[offset];
            offset += 1;
            current.copy_from_slice(&inflated[offset..offset + row_bytes]);
            offset += row_bytes;
            png_unfilter_row(filter, bytes_per_pixel, &previous, &mut current)?;
            decode_png_row_to_rgba(parsed, &current, pass_width, &mut row_rgba)?;
            let final_y = y_start + pass_y * y_step;
            for pass_x in 0..pass_width {
                let final_x = x_start + pass_x * x_step;
                let out = (final_y * width + final_x) * 4;
                let src = pass_x * 4;
                rgba[out..out + 4].copy_from_slice(&row_rgba[src..src + 4]);
            }
            previous.copy_from_slice(&current);
        }
    }
    if offset != inflated.len() {
        return Err("PNG Adam7 decoded scanline byte count has trailing bytes");
    }
    Ok(())
}

fn adam7_axis_count(size: usize, start: usize, step: usize) -> usize {
    if size <= start {
        0
    } else {
        (size - start).div_ceil(step)
    }
}

fn decode_png_row_to_rgba(
    parsed: &ParsedPngData,
    row: &[u8],
    width: usize,
    out: &mut [u8],
) -> Result<(), &'static str> {
    match parsed.header.colour_type {
        0 => decode_png_grayscale_row(parsed, row, width, out),
        2 => decode_png_true_colour_row(parsed, row, width, out),
        3 => decode_png_indexed_row(parsed, row, width, out),
        4 => decode_png_grayscale_alpha_row(parsed, row, width, out),
        6 => {
            if out.len() < width.saturating_mul(4) {
                return Err("PNG RGBA row output is too short");
            }
            match parsed.header.bit_depth {
                8 => {
                    let byte_count = width
                        .checked_mul(4)
                        .ok_or("PNG RGBA row byte count overflows")?;
                    if row.len() < byte_count {
                        return Err("PNG RGBA row is truncated");
                    }
                    out[..byte_count].copy_from_slice(&row[..byte_count]);
                }
                16 => {
                    for x in 0..width {
                        let input = x * 8;
                        let Some(bytes) = row.get(input..input + 8) else {
                            return Err("PNG RGBA row is truncated");
                        };
                        // Down-sample each 16-bit channel to its high byte.
                        let offset = x * 4;
                        out[offset..offset + 4]
                            .copy_from_slice(&[bytes[0], bytes[2], bytes[4], bytes[6]]);
                    }
                }
                _ => return Err("PNG RGBA bit depth is unsupported"),
            }
            Ok(())
        }
        _ => Err("PNG colour type is not implemented"),
    }
}

fn inspect_standard_png_graphic_data(data: &[u8]) -> Result<PngHeader, &'static str> {
    Ok(parse_standard_png_graphic_data(data)?.header)
}

struct ParsedPngData {
    header: PngHeader,
    interlace: u8,
    idat: Vec<u8>,
    palette: Vec<[u8; 3]>,
    transparency: Vec<u8>,
}

fn parse_standard_png_graphic_data(data: &[u8]) -> Result<ParsedPngData, &'static str> {
    if data.len() < 33 {
        return Err("payload is too short for PNG signature and IHDR");
    }
    if &data[..8] != PNG_SIGNATURE {
        return Err("missing PNG signature");
    }
    let ihdr_len = png_be_u32(data, 8).ok_or("truncated IHDR length")?;
    if ihdr_len != 13 {
        return Err("first chunk is not a 13-byte IHDR");
    }
    if data.get(12..16) != Some(b"IHDR") {
        return Err("first chunk is not IHDR");
    }
    let ihdr_end = 16usize
        .checked_add(13)
        .and_then(|end| end.checked_add(4))
        .ok_or("IHDR length overflows")?;
    if ihdr_end > data.len() {
        return Err("IHDR extends past payload");
    }
    validate_png_chunk_crc(data, 12, 29, 33)?;
    let width = png_be_u32(data, 16).ok_or("truncated IHDR width")?;
    let height = png_be_u32(data, 20).ok_or("truncated IHDR height")?;
    if width == 0 || height == 0 {
        return Err("IHDR width/height must be non-zero");
    }
    let bit_depth = data[24];
    let colour_type = data[25];
    let compression = data[26];
    let filter = data[27];
    let interlace = data[28];
    if compression != 0 || filter != 0 {
        return Err("IHDR compression/filter method is unsupported");
    }
    if interlace > 1 {
        return Err("IHDR interlace method is invalid");
    }
    let _ = png_bits_per_pixel(bit_depth, colour_type)
        .ok_or("IHDR colour type/bit depth is invalid")?;

    let mut offset = ihdr_end;
    let mut seen_idat = false;
    let mut idat = Vec::new();
    let mut palette = Vec::new();
    let mut transparency = Vec::new();
    loop {
        if offset == data.len() {
            return Err("PNG payload has no IEND chunk");
        }
        if offset + 12 > data.len() {
            return Err("PNG chunk header extends past payload");
        }
        let chunk_len = png_be_u32(data, offset).ok_or("truncated PNG chunk length")? as usize;
        let chunk_type_start = offset + 4;
        let chunk_data_start = offset + 8;
        let Some(chunk_data_end) = chunk_data_start.checked_add(chunk_len) else {
            return Err("PNG chunk length overflows");
        };
        let Some(chunk_end) = chunk_data_end.checked_add(4) else {
            return Err("PNG chunk length overflows");
        };
        if chunk_end > data.len() {
            return Err("PNG chunk extends past payload");
        }
        let chunk_type = &data[chunk_type_start..chunk_data_start];
        validate_png_chunk_crc(data, chunk_type_start, chunk_data_end, chunk_end)?;
        if chunk_type == b"IDAT" {
            seen_idat = true;
            idat.extend_from_slice(&data[chunk_data_start..chunk_data_end]);
        } else if chunk_type == b"PLTE" {
            if seen_idat {
                return Err("PNG PLTE chunk appears after IDAT");
            }
            if chunk_len == 0 || !chunk_len.is_multiple_of(3) || chunk_len / 3 > 256 {
                return Err("PNG PLTE chunk length is invalid");
            }
            palette.clear();
            for rgb in data[chunk_data_start..chunk_data_end].chunks_exact(3) {
                palette.push([rgb[0], rgb[1], rgb[2]]);
            }
        } else if chunk_type == b"tRNS" {
            if seen_idat {
                return Err("PNG tRNS chunk appears after IDAT");
            }
            transparency.clear();
            transparency.extend_from_slice(&data[chunk_data_start..chunk_data_end]);
        } else if chunk_type == b"IEND" {
            if chunk_len != 0 {
                return Err("IEND chunk must be empty");
            }
            if !seen_idat {
                return Err("PNG payload has no IDAT chunk");
            }
            if chunk_end != data.len() {
                return Err("PNG payload has trailing bytes after IEND");
            }
            if colour_type == 3 {
                if palette.is_empty() {
                    return Err("indexed-colour PNG is missing PLTE");
                }
                if transparency.len() > palette.len() {
                    return Err("indexed-colour PNG tRNS exceeds PLTE length");
                }
                let max_entries = 1usize << usize::from(bit_depth);
                if palette.len() > max_entries {
                    return Err("indexed-colour PNG PLTE has more entries than bit depth permits");
                }
            }
            match colour_type {
                0 if !transparency.is_empty() && transparency.len() != 2 => {
                    return Err("grayscale PNG tRNS chunk length is invalid");
                }
                2 if !transparency.is_empty() && transparency.len() != 6 => {
                    return Err("true-colour PNG tRNS chunk length is invalid");
                }
                4 | 6 if !transparency.is_empty() => {
                    return Err("PNG tRNS chunk is invalid for alpha colour types");
                }
                _ => {}
            }
            return Ok(ParsedPngData {
                header: PngHeader {
                    width,
                    height,
                    bit_depth,
                    colour_type,
                },
                interlace,
                idat,
                palette,
                transparency,
            });
        }
        offset = chunk_end;
    }
}

fn validate_png_chunk_crc(
    data: &[u8],
    chunk_type_start: usize,
    chunk_data_end: usize,
    chunk_end: usize,
) -> Result<(), &'static str> {
    let crc_bytes: [u8; 4] = data
        .get(chunk_data_end..chunk_end)
        .ok_or("PNG chunk CRC extends past payload")?
        .try_into()
        .map_err(|_| "PNG chunk CRC is truncated")?;
    let expected = u32::from_be_bytes(crc_bytes);
    let computed = png_crc32(
        data.get(chunk_type_start..chunk_data_end)
            .ok_or("PNG chunk CRC input extends past payload")?,
    );
    if computed != expected {
        return Err("PNG chunk CRC check failed");
    }
    Ok(())
}

fn png_be_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

fn png_crc32(bytes: &[u8]) -> u32 {
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

fn png_bits_per_pixel(bit_depth: u8, colour_type: u8) -> Option<u8> {
    let samples = match colour_type {
        0 if matches!(bit_depth, 1 | 2 | 4 | 8 | 16) => 1,
        2 if matches!(bit_depth, 8 | 16) => 3,
        3 if matches!(bit_depth, 1 | 2 | 4 | 8) => 1,
        4 if matches!(bit_depth, 8 | 16) => 2,
        6 if matches!(bit_depth, 8 | 16) => 4,
        _ => return None,
    };
    Some(samples * bit_depth)
}

fn png_scanline_bytes(width: u16, bit_depth: u8, colour_type: u8) -> Option<usize> {
    let bits_per_pixel = usize::from(png_bits_per_pixel(bit_depth, colour_type)?);
    let bits = usize::from(width).checked_mul(bits_per_pixel)?;
    Some(bits.saturating_add(7) / 8)
}

fn png_filter_bytes_per_pixel(bit_depth: u8, colour_type: u8) -> Option<usize> {
    let bits_per_pixel = usize::from(png_bits_per_pixel(bit_depth, colour_type)?);
    Some(bits_per_pixel.saturating_add(7).max(8) / 8)
}

fn decode_png_grayscale_row(
    parsed: &ParsedPngData,
    row: &[u8],
    width: usize,
    out: &mut [u8],
) -> Result<(), &'static str> {
    if out.len() < width.saturating_mul(4) {
        return Err("PNG grayscale row output is too short");
    }
    let transparent = png_trns_gray(&parsed.transparency)?;
    for x in 0..width {
        let raw = png_gray_raw_sample(row, parsed.header.bit_depth, x)
            .ok_or("PNG grayscale row is truncated")?;
        let gray = png_gray_sample_to_u8(raw, parsed.header.bit_depth)
            .ok_or("PNG grayscale bit depth is unsupported")?;
        let alpha = if transparent == Some(raw) { 0x00 } else { 0xFF };
        let offset = x * 4;
        out[offset..offset + 4].copy_from_slice(&[gray, gray, gray, alpha]);
    }
    Ok(())
}

fn decode_png_grayscale_alpha_row(
    parsed: &ParsedPngData,
    row: &[u8],
    width: usize,
    out: &mut [u8],
) -> Result<(), &'static str> {
    if out.len() < width.saturating_mul(4) {
        return Err("PNG grayscale-alpha row output is too short");
    }
    match parsed.header.bit_depth {
        8 => {
            for x in 0..width {
                let input = x * 2;
                let Some((&gray, &alpha)) = row.get(input).zip(row.get(input + 1)) else {
                    return Err("PNG grayscale-alpha row is truncated");
                };
                let offset = x * 4;
                out[offset..offset + 4].copy_from_slice(&[gray, gray, gray, alpha]);
            }
        }
        16 => {
            for x in 0..width {
                let input = x * 4;
                let Some(bytes) = row.get(input..input + 4) else {
                    return Err("PNG grayscale-alpha row is truncated");
                };
                let gray = bytes[0];
                let alpha = bytes[2];
                let offset = x * 4;
                out[offset..offset + 4].copy_from_slice(&[gray, gray, gray, alpha]);
            }
        }
        _ => return Err("PNG grayscale-alpha bit depth is unsupported"),
    }
    Ok(())
}

fn decode_png_true_colour_row(
    parsed: &ParsedPngData,
    row: &[u8],
    width: usize,
    out: &mut [u8],
) -> Result<(), &'static str> {
    if out.len() < width.saturating_mul(4) {
        return Err("PNG true-colour row output is too short");
    }
    let transparent_rgb = png_trns_rgb(&parsed.transparency)?;
    match parsed.header.bit_depth {
        8 => {
            if row.len() < width.saturating_mul(3) {
                return Err("PNG true-colour row is truncated");
            }
            for x in 0..width {
                let input = x * 3;
                let output = x * 4;
                let alpha = if transparent_rgb
                    == Some((
                        u16::from(row[input]),
                        u16::from(row[input + 1]),
                        u16::from(row[input + 2]),
                    )) {
                    0x00
                } else {
                    0xFF
                };
                out[output..output + 4].copy_from_slice(&[
                    row[input],
                    row[input + 1],
                    row[input + 2],
                    alpha,
                ]);
            }
        }
        16 => {
            for x in 0..width {
                let input = x * 6;
                let Some(bytes) = row.get(input..input + 6) else {
                    return Err("PNG true-colour row is truncated");
                };
                // Down-sample each 16-bit channel to its high byte; the tRNS
                // comparison uses the full 16-bit samples.
                let sample = |i: usize| u16::from_be_bytes([bytes[i], bytes[i + 1]]);
                let alpha = if transparent_rgb == Some((sample(0), sample(2), sample(4))) {
                    0x00
                } else {
                    0xFF
                };
                let output = x * 4;
                out[output..output + 4].copy_from_slice(&[bytes[0], bytes[2], bytes[4], alpha]);
            }
        }
        _ => return Err("PNG true-colour bit depth is unsupported"),
    }
    Ok(())
}

fn decode_png_indexed_row(
    parsed: &ParsedPngData,
    row: &[u8],
    width: usize,
    out: &mut [u8],
) -> Result<(), &'static str> {
    if out.len() < width.saturating_mul(4) {
        return Err("PNG indexed row output is too short");
    }
    for x in 0..width {
        let index = png_packed_sample(row, parsed.header.bit_depth, x)
            .ok_or("PNG indexed row is truncated")?;
        let Some(rgb) = parsed.palette.get(usize::from(index)).copied() else {
            return Err("PNG indexed pixel references a missing PLTE entry");
        };
        let alpha = parsed
            .transparency
            .get(usize::from(index))
            .copied()
            .unwrap_or(0xFF);
        let offset = x * 4;
        out[offset..offset + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
    }
    Ok(())
}

fn png_trns_gray(data: &[u8]) -> Result<Option<u16>, &'static str> {
    if data.is_empty() {
        return Ok(None);
    }
    let bytes: [u8; 2] = data
        .try_into()
        .map_err(|_| "grayscale PNG tRNS chunk length is invalid")?;
    Ok(Some(u16::from_be_bytes(bytes)))
}

fn png_trns_rgb(data: &[u8]) -> Result<Option<(u16, u16, u16)>, &'static str> {
    if data.is_empty() {
        return Ok(None);
    }
    if data.len() != 6 {
        return Err("true-colour PNG tRNS chunk length is invalid");
    }
    Ok(Some((
        u16::from_be_bytes([data[0], data[1]]),
        u16::from_be_bytes([data[2], data[3]]),
        u16::from_be_bytes([data[4], data[5]]),
    )))
}

fn png_gray_raw_sample(row: &[u8], bit_depth: u8, x: usize) -> Option<u16> {
    match bit_depth {
        1 | 2 | 4 | 8 => png_packed_sample(row, bit_depth, x).map(u16::from),
        16 => {
            let offset = x.checked_mul(2)?;
            let bytes: [u8; 2] = row.get(offset..offset + 2)?.try_into().ok()?;
            Some(u16::from_be_bytes(bytes))
        }
        _ => None,
    }
}

fn png_gray_sample_to_u8(sample: u16, bit_depth: u8) -> Option<u8> {
    match bit_depth {
        1 => Some(if sample == 0 { 0 } else { 0xFF }),
        2 => Some(u8::try_from(sample.saturating_mul(85)).ok()?),
        4 => Some(u8::try_from(sample.saturating_mul(17)).ok()?),
        8 => u8::try_from(sample).ok(),
        16 => Some((sample >> 8) as u8),
        _ => None,
    }
}

fn png_packed_sample(row: &[u8], bit_depth: u8, x: usize) -> Option<u8> {
    match bit_depth {
        1 => {
            let byte = *row.get(x / 8)?;
            Some((byte >> (7 - (x % 8))) & 0x01)
        }
        2 => {
            let byte = *row.get(x / 4)?;
            Some((byte >> (6 - (x % 4) * 2)) & 0x03)
        }
        4 => {
            let byte = *row.get(x / 2)?;
            Some(if x.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0F
            })
        }
        8 => row.get(x).copied(),
        _ => None,
    }
}
