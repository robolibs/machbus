use crate::isobus::vt::ObjectID;
use crate::isobus::vt::render::gtui::{GraphicsContextCopySource, GtuiRenderer, RenderCommand};
use crate::isobus::vt::render::runtime::VtRenderRuntime;
use crate::isobus::vt::render::scene::{FillPattern, Rect, Scene};
use crate::isobus::vt::render::style::{Colour, FontDecoration, FontMetrics, Palette};
use crate::isobus::vt::render::text::{self, HorizontalAlign, TextLayout, VerticalAlign};

const MAX_FRAMEBUFFER_PIXELS: usize = 8 * 1024 * 1024;
const PLACEHOLDER_COLOUR: Colour = Colour::rgb(255, 0, 255);

/// In-memory RGB framebuffer produced by [`FramebufferRenderer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Framebuffer {
    width: u16,
    height: u16,
    pixels: Vec<Colour>,
}

impl Framebuffer {
    /// Create a framebuffer, refusing oversized dimensions instead of risking
    /// an unbounded allocation.
    #[must_use]
    pub fn try_new(width: u16, height: u16, background: Colour) -> Option<Self> {
        let len = usize::from(width).checked_mul(usize::from(height))?;
        if len > MAX_FRAMEBUFFER_PIXELS {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: vec![background; len],
        })
    }

    #[inline]
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    #[inline]
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    #[inline]
    #[must_use]
    pub fn pixels(&self) -> &[Colour] {
        &self.pixels
    }

    /// Number of bytes required for an RGB888 export.
    #[inline]
    #[must_use]
    pub fn rgb888_len(&self) -> usize {
        self.pixels.len().saturating_mul(3)
    }

    /// Number of bytes required for an RGB565 export.
    #[inline]
    #[must_use]
    pub fn rgb565_len(&self) -> usize {
        self.pixels.len().saturating_mul(2)
    }

    /// Export pixels as tightly packed RGB888 bytes.
    #[must_use]
    pub fn to_rgb888(&self) -> Vec<u8> {
        let mut out = vec![0; self.rgb888_len()];
        self.write_rgb888(&mut out)
            .expect("fresh RGB888 export buffer is correctly sized");
        out
    }

    /// Export pixels as big-endian RGB565 bytes.
    ///
    /// Big-endian is the common wire order for many SPI display controllers.
    #[must_use]
    pub fn to_rgb565_be(&self) -> Vec<u8> {
        let mut out = vec![0; self.rgb565_len()];
        self.write_rgb565_be(&mut out)
            .expect("fresh RGB565 export buffer is correctly sized");
        out
    }

    /// Export pixels as little-endian RGB565 bytes.
    #[must_use]
    pub fn to_rgb565_le(&self) -> Vec<u8> {
        let mut out = vec![0; self.rgb565_len()];
        self.write_rgb565_le(&mut out)
            .expect("fresh RGB565 export buffer is correctly sized");
        out
    }

    /// Write tightly packed RGB888 bytes into a caller-provided buffer.
    pub fn write_rgb888(&self, out: &mut [u8]) -> Result<(), FramebufferExportError> {
        let required = self.rgb888_len();
        if out.len() < required {
            return Err(FramebufferExportError::BufferTooSmall {
                required,
                available: out.len(),
            });
        }
        for (pixel, chunk) in self.pixels.iter().zip(out[..required].chunks_exact_mut(3)) {
            chunk.copy_from_slice(&[pixel.r, pixel.g, pixel.b]);
        }
        Ok(())
    }

    /// Write big-endian RGB565 bytes into a caller-provided buffer.
    pub fn write_rgb565_be(&self, out: &mut [u8]) -> Result<(), FramebufferExportError> {
        self.write_rgb565(out, u16::to_be_bytes)
    }

    /// Write little-endian RGB565 bytes into a caller-provided buffer.
    pub fn write_rgb565_le(&self, out: &mut [u8]) -> Result<(), FramebufferExportError> {
        self.write_rgb565(out, u16::to_le_bytes)
    }

    /// Read one pixel. Out-of-bounds coordinates return `None`.
    #[must_use]
    pub fn pixel(&self, x: u16, y: u16) -> Option<Colour> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.pixels
            .get(usize::from(y) * usize::from(self.width) + usize::from(x))
            .copied()
    }

    /// Count pixels with an exact RGB match. Useful for snapshot tests that do
    /// not care about full image bytes.
    #[must_use]
    pub fn count_colour(&self, colour: Colour) -> usize {
        self.pixels.iter().filter(|&&pixel| pixel == colour).count()
    }

    fn set_pixel(&mut self, x: i32, y: i32, colour: Colour, clip: Rect) {
        if x < 0 || y < 0 || !clip.contains(x, y) {
            return;
        }
        let Ok(x) = u16::try_from(x) else {
            return;
        };
        let Ok(y) = u16::try_from(y) else {
            return;
        };
        if x >= self.width || y >= self.height {
            return;
        }
        let index = usize::from(y) * usize::from(self.width) + usize::from(x);
        if let Some(pixel) = self.pixels.get_mut(index) {
            *pixel = colour;
        }
    }

    fn write_rgb565(
        &self,
        out: &mut [u8],
        byte_order: impl Fn(u16) -> [u8; 2],
    ) -> Result<(), FramebufferExportError> {
        let required = self.rgb565_len();
        if out.len() < required {
            return Err(FramebufferExportError::BufferTooSmall {
                required,
                available: out.len(),
            });
        }
        for (pixel, chunk) in self.pixels.iter().zip(out[..required].chunks_exact_mut(2)) {
            chunk.copy_from_slice(&byte_order(rgb565(*pixel)));
        }
        Ok(())
    }
}

/// Error returned by framebuffer pixel export methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferExportError {
    /// The caller-provided output slice is smaller than the encoded frame.
    BufferTooSmall { required: usize, available: usize },
}

/// Error returned by fallible framebuffer rendering methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FramebufferError {
    /// The command stream has no positive drawable bounds, or the auto-sized
    /// bounds exceed the framebuffer coordinate type.
    InvalidCommandBounds,
    /// The requested framebuffer would exceed the backend allocation guard.
    FrameTooLarge,
}

#[inline]
#[must_use]
fn rgb565(pixel: Colour) -> u16 {
    (u16::from(pixel.r & 0xF8) << 8) | (u16::from(pixel.g & 0xFC) << 3) | (u16::from(pixel.b) >> 3)
}

/// Software framebuffer renderer for backend-neutral VT commands.
#[derive(Debug, Clone)]
pub struct FramebufferRenderer {
    command_renderer: GtuiRenderer,
    background: Colour,
}

impl Default for FramebufferRenderer {
    fn default() -> Self {
        Self {
            command_renderer: GtuiRenderer::default(),
            background: Colour::rgb(255, 255, 255),
        }
    }
}

impl FramebufferRenderer {
    #[must_use]
    pub fn new(command_renderer: GtuiRenderer, background: Colour) -> Self {
        Self {
            command_renderer,
            background,
        }
    }

    /// Render a retained VT scene by first lowering it through the GTUI command
    /// stream and then rasterising those backend-neutral commands.
    ///
    /// The framebuffer is auto-sized to the positive command bounds.
    #[must_use]
    pub fn render_scene(&self, scene: &Scene) -> Option<Framebuffer> {
        self.try_render_scene(scene).ok()
    }

    /// Fallible form of [`Self::render_scene`] with a concrete error reason.
    pub fn try_render_scene(&self, scene: &Scene) -> Result<Framebuffer, FramebufferError> {
        let commands = self.command_renderer.render(scene);
        match &scene.effective_palette {
            Some(palette) => self.try_render_commands_auto_sized_with_palette(&commands, palette),
            None => self.try_render_commands_auto_sized(&commands),
        }
    }

    /// Render a live VT runtime snapshot, including runtime-only command-stream
    /// additions such as accepted Graphics Context replay/primitive expansion.
    ///
    /// Use this instead of [`Self::render_scene`] when the source of truth is a
    /// [`VtRenderRuntime`]: a retained [`Scene`] alone does not contain the
    /// ordered ECU-to-VT Graphics Context commands that the runtime appends to
    /// the backend-neutral command stream.
    #[must_use]
    pub fn render_runtime(&self, runtime: &VtRenderRuntime) -> Option<Framebuffer> {
        self.try_render_runtime(runtime).ok()
    }

    /// Fallible form of [`Self::render_runtime`] with a concrete error reason.
    pub fn try_render_runtime(
        &self,
        runtime: &VtRenderRuntime,
    ) -> Result<Framebuffer, FramebufferError> {
        let commands = runtime.render_commands(&self.command_renderer);
        match &runtime.scene().effective_palette {
            Some(palette) => self.try_render_commands_auto_sized_with_palette(&commands, palette),
            None => self.try_render_commands_auto_sized(&commands),
        }
    }

    /// Rasterise commands into a caller-sized framebuffer.
    #[must_use]
    pub fn render_commands(
        &self,
        width: u16,
        height: u16,
        commands: &[RenderCommand],
    ) -> Option<Framebuffer> {
        self.try_render_commands(width, height, commands).ok()
    }

    /// Fallible form of [`Self::render_commands`] with a concrete error reason.
    pub fn try_render_commands(
        &self,
        width: u16,
        height: u16,
        commands: &[RenderCommand],
    ) -> Result<Framebuffer, FramebufferError> {
        self.try_render_commands_with_palette(
            width,
            height,
            commands,
            self.command_renderer.palette(),
        )
    }

    fn try_render_commands_with_palette(
        &self,
        width: u16,
        height: u16,
        commands: &[RenderCommand],
        palette: &Palette,
    ) -> Result<Framebuffer, FramebufferError> {
        let mut fb = Framebuffer::try_new(width, height, self.background)
            .ok_or(FramebufferError::FrameTooLarge)?;
        let mut clip = Rect::new(0, 0, width, height);
        let mut picture_updates = Vec::new();
        let mut graphics_contexts = Vec::new();
        for command in commands {
            record_picture_update(command, &mut picture_updates);
            Self::apply_command(
                &mut fb,
                command,
                &mut clip,
                &mut picture_updates,
                &mut graphics_contexts,
                palette,
            );
        }
        Ok(fb)
    }

    /// Rasterise commands into the smallest framebuffer that contains their
    /// positive draw bounds.
    #[must_use]
    pub fn render_commands_auto_sized(&self, commands: &[RenderCommand]) -> Option<Framebuffer> {
        self.try_render_commands_auto_sized(commands).ok()
    }

    /// Fallible form of [`Self::render_commands_auto_sized`] with a concrete
    /// error reason.
    pub fn try_render_commands_auto_sized(
        &self,
        commands: &[RenderCommand],
    ) -> Result<Framebuffer, FramebufferError> {
        let (width, height) =
            command_bounds(commands).ok_or(FramebufferError::InvalidCommandBounds)?;
        self.try_render_commands(width, height, commands)
    }

    fn try_render_commands_auto_sized_with_palette(
        &self,
        commands: &[RenderCommand],
        palette: &Palette,
    ) -> Result<Framebuffer, FramebufferError> {
        let (width, height) =
            command_bounds(commands).ok_or(FramebufferError::InvalidCommandBounds)?;
        self.try_render_commands_with_palette(width, height, commands, palette)
    }

    fn apply_command(
        fb: &mut Framebuffer,
        command: &RenderCommand,
        clip: &mut Rect,
        picture_updates: &mut Vec<PictureUpdate>,
        graphics_contexts: &mut Vec<FramebufferGraphicsContextState>,
        palette: &Palette,
    ) {
        match command {
            RenderCommand::Clip(rect) => *clip = framebuffer_clip(*rect, fb.width, fb.height),
            RenderCommand::FillRect { rect, colour } => fill_rect(fb, *rect, *colour, *clip),
            RenderCommand::StrokeRect {
                rect,
                colour,
                width,
                line_art,
                suppress,
            } => {
                if *width != 0 {
                    stroke_rect(fb, *rect, *colour, *width, *line_art, *suppress, *clip);
                }
            }
            RenderCommand::Line {
                x0,
                y0,
                x1,
                y1,
                colour,
                width,
                line_art,
            } => {
                if *width != 0 {
                    draw_line(
                        fb,
                        LineStroke {
                            x0: *x0,
                            y0: *y0,
                            x1: *x1,
                            y1: *y1,
                            colour: *colour,
                            width: *width,
                            line_art: *line_art,
                        },
                        *clip,
                    );
                }
            }
            RenderCommand::Ellipse {
                rect,
                colour,
                fill_colour,
                filled,
                width,
                line_art,
            } => draw_ellipse(
                fb,
                *rect,
                ShapeDrawStyle {
                    line_colour: *colour,
                    fill_colour: *fill_colour,
                    filled: *filled,
                    line_enabled: *width != 0,
                    line_width: *width,
                    line_art: *line_art,
                },
                *clip,
            ),
            RenderCommand::EllipseArc {
                rect,
                colour,
                fill_colour,
                filled,
                width,
                line_art,
                ellipse_type,
                start_angle,
                end_angle,
            } => draw_ellipse_arc(
                fb,
                EllipseArcDraw {
                    rect: *rect,
                    line_colour: *colour,
                    fill_colour: *fill_colour,
                    filled: *filled,
                    line_enabled: *width != 0,
                    ellipse_type: *ellipse_type,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                    width: *width,
                    line_art: *line_art,
                },
                *clip,
            ),
            RenderCommand::Polygon {
                points,
                colour,
                fill_colour,
                filled,
                width,
                line_art,
                ..
            } => draw_polygon(
                fb,
                points,
                ShapeDrawStyle {
                    line_colour: *colour,
                    fill_colour: *fill_colour,
                    filled: *filled,
                    line_enabled: *width != 0,
                    line_width: *width,
                    line_art: *line_art,
                },
                *clip,
            ),
            RenderCommand::PatternFillRect {
                rect,
                anchor,
                pattern,
            } => {
                draw_pattern_rect(fb, *rect, *anchor, pattern, palette, *clip);
            }
            RenderCommand::PatternFillEllipse {
                rect,
                anchor,
                ellipse_type,
                start_angle,
                end_angle,
                pattern,
            } => draw_pattern_ellipse(
                fb,
                PatternEllipseDraw {
                    rect: *rect,
                    anchor: *anchor,
                    ellipse_type: *ellipse_type,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                    pattern,
                    palette,
                },
                *clip,
            ),
            RenderCommand::PatternFillPolygon {
                points,
                anchor,
                pattern,
                ..
            } => draw_pattern_polygon(fb, points, *anchor, pattern, palette, *clip),
            RenderCommand::DrawText {
                rect,
                style,
                layout,
                ..
            } => draw_text_cells(
                fb,
                *rect,
                TextCellStyle {
                    foreground: style.foreground,
                    background: style.background,
                    metrics: style.font,
                    decoration: style.decoration,
                },
                layout,
                *clip,
            ),
            RenderCommand::Meter {
                rect,
                value,
                min,
                max,
                needle_colour,
                border_colour,
                arc_colour,
                show_value,
                number_of_ticks,
                start_angle,
                end_angle,
                ..
            } => draw_meter(
                fb,
                MeterDraw {
                    rect: *rect,
                    value: *value,
                    min: *min,
                    max: *max,
                    needle_colour: *needle_colour,
                    border_colour: *border_colour,
                    arc_colour: *arc_colour,
                    show_value: *show_value,
                    number_of_ticks: *number_of_ticks,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                },
                *clip,
            ),
            RenderCommand::BarGraph {
                rect,
                value,
                target_value,
                min,
                max,
                colour,
                target_line_colour,
                show_border,
                show_target_line,
                show_ticks,
                number_of_ticks,
                line_only,
                arched,
                horizontal,
                direction_positive,
                clockwise,
                start_angle,
                end_angle,
                bar_width,
                ..
            } => draw_bar_graph(
                fb,
                BarGraphDraw {
                    rect: *rect,
                    value: *value,
                    target_value: *target_value,
                    min: *min,
                    max: *max,
                    colour: *colour,
                    target_line_colour: *target_line_colour,
                    show_border: *show_border,
                    show_target_line: *show_target_line,
                    show_ticks: *show_ticks,
                    number_of_ticks: *number_of_ticks,
                    line_only: *line_only,
                    arched: *arched,
                    horizontal: *horizontal,
                    direction_positive: *direction_positive,
                    clockwise: *clockwise,
                    start_angle: *start_angle,
                    end_angle: *end_angle,
                    bar_width: *bar_width,
                },
                *clip,
            ),
            RenderCommand::IndexedImage {
                object_id,
                rect,
                width,
                height,
                format,
                transparent,
                transparency,
                data,
            } => {
                if let Some(update) = picture_update_for(picture_updates, *object_id) {
                    draw_updated_indexed_image(
                        fb,
                        UpdatedIndexedImageDraw {
                            rect: *rect,
                            update_width: update.width,
                            update_height: update.height,
                            update_format: update.format,
                            update_transparent_index: update.transparent_index,
                            update_data: &update.data,
                            base_width: *width,
                            base_height: *height,
                            base_format: *format,
                            base_transparent: *transparent,
                            base_transparency: *transparency,
                            base_data: data,
                            palette,
                        },
                        *clip,
                    );
                } else {
                    draw_indexed_image(
                        fb,
                        IndexedImageDraw {
                            rect: *rect,
                            source_width: *width,
                            source_height: *height,
                            format: *format,
                            transparent: *transparent,
                            transparency: *transparency,
                            data,
                            palette,
                        },
                        *clip,
                    );
                }
            }
            RenderCommand::RgbaImage {
                rect,
                width,
                height,
                data,
                ..
            } => draw_rgba_image(
                fb,
                RgbaImageDraw {
                    rect: *rect,
                    source_width: *width,
                    source_height: *height,
                    data,
                },
                *clip,
            ),
            RenderCommand::Placeholder { rect, .. } => {
                if rect.w != 0 && rect.h != 0 {
                    fill_rect(fb, *rect, PLACEHOLDER_COLOUR, *clip);
                }
            }
            RenderCommand::SoftKey {
                rect, label, style, ..
            } => {
                fill_rect(fb, *rect, style.background, *clip);
                stroke_rect(fb, *rect, style.foreground, 1, 0xFFFF, 0, *clip);
                draw_label_cells(fb, *rect, style.foreground, label, *clip);
            }
            RenderCommand::GraphicsContextCanvas {
                object_id,
                rect,
                canvas_width,
                canvas_height,
                background,
                transparency_colour,
                transparent,
                ..
            } => {
                graphics_context_state_for(graphics_contexts, *object_id, *rect).set_canvas(
                    *rect,
                    *canvas_width,
                    *canvas_height,
                    *background,
                    *transparency_colour,
                    *transparent,
                );
                if !transparent {
                    fill_rect(fb, *rect, palette.resolve(*background), *clip);
                }
                stroke_rect(fb, *rect, Colour::gray(96), 1, 0xFFFF, 0, *clip);
            }
            RenderCommand::GraphicsContextViewport {
                object_id,
                viewport,
                zoom_raw,
                ..
            } => {
                let state = graphics_context_state_for(graphics_contexts, *object_id, *viewport);
                state.viewport = *viewport;
                state.zoom_raw = *zoom_raw;
            }
            RenderCommand::GraphicsContextReplay {
                object_id,
                subcommand,
                payload,
            } => apply_graphics_context_replay(
                FramebufferGraphicsContextReplayTarget {
                    fb,
                    contexts: graphics_contexts,
                    picture_updates,
                    palette,
                    clip: *clip,
                },
                *object_id,
                *subcommand,
                payload,
            ),
            RenderCommand::GraphicsContextCopyToPicture {
                object_id,
                picture_id,
                source,
                viewport,
                zoom_raw,
                ..
            } => record_framebuffer_copy_to_picture(
                graphics_contexts,
                FramebufferCopyRequest {
                    object_id: *object_id,
                    picture_id: *picture_id,
                    source: *source,
                    viewport: *viewport,
                    zoom_raw: *zoom_raw,
                },
                picture_updates,
            ),
            RenderCommand::GraphicsContextPictureData { .. } => {}
        }
    }
}

const MAX_FRAMEBUFFER_GRAPHICS_CONTEXT_PIXELS: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
struct FramebufferGraphicsContextState {
    object_id: ObjectID,
    canvas: Rect,
    canvas_width: u16,
    canvas_height: u16,
    canvas_pixels: Vec<u8>,
    viewport: Rect,
    cursor_x: i32,
    cursor_y: i32,
    foreground: u8,
    background: u8,
    transparency_colour: u8,
    text_foreground: u8,
    font_attributes_selected: bool,
    font: FontMetrics,
    decoration: FontDecoration,
    line_width: u16,
    line_enabled: bool,
    fill_enabled: bool,
    zoom_raw: Option<u32>,
}

impl FramebufferGraphicsContextState {
    fn new(object_id: ObjectID, viewport: Rect) -> Self {
        Self {
            object_id,
            canvas: viewport,
            canvas_width: viewport.w,
            canvas_height: viewport.h,
            canvas_pixels: graphics_context_canvas_pixels(viewport.w, viewport.h, 0),
            viewport,
            cursor_x: 0,
            cursor_y: 0,
            foreground: 1,
            background: 0,
            transparency_colour: 0,
            text_foreground: 1,
            font_attributes_selected: false,
            font: FontMetrics::default(),
            decoration: FontDecoration::default(),
            line_width: 1,
            line_enabled: true,
            fill_enabled: false,
            zoom_raw: None,
        }
    }

    const fn device_point(&self) -> (i32, i32) {
        (
            self.viewport.x + self.cursor_x,
            self.viewport.y + self.cursor_y,
        )
    }

    const fn resolved_text_foreground(&self) -> u8 {
        if self.font_attributes_selected {
            self.text_foreground
        } else {
            self.foreground
        }
    }

    const fn rect_at_cursor(&self, w: u16, h: u16) -> Rect {
        let (x, y) = self.device_point();
        Rect::new(x, y, w, h)
    }

    fn move_to_bottom_right_inside(&mut self, w: u16, h: u16) {
        self.cursor_x = self.cursor_x.saturating_add(i32::from(w.saturating_sub(1)));
        self.cursor_y = self.cursor_y.saturating_add(i32::from(h.saturating_sub(1)));
    }

    fn set_canvas(
        &mut self,
        canvas: Rect,
        canvas_width: u16,
        canvas_height: u16,
        background: u8,
        transparency_colour: u8,
        transparent: bool,
    ) {
        self.canvas = canvas;
        self.viewport = canvas;
        self.canvas_width = canvas_width;
        self.canvas_height = canvas_height;
        self.background = background;
        self.transparency_colour = transparency_colour;
        self.canvas_pixels = graphics_context_canvas_pixels(
            canvas_width,
            canvas_height,
            if transparent {
                transparency_colour
            } else {
                background
            },
        );
    }

    fn set_viewport_position(&mut self, x: i32, y: i32) {
        self.viewport.x = self.canvas.x.saturating_add(x);
        self.viewport.y = self.canvas.y.saturating_add(y);
    }

    const fn set_viewport_size(&mut self, w: u16, h: u16) {
        self.viewport.w = w;
        self.viewport.h = h;
    }

    fn canvas_set_pixel_at_screen_point(&mut self, x: i32, y: i32, colour: u8) {
        let local_x = x.saturating_sub(self.canvas.x);
        let local_y = y.saturating_sub(self.canvas.y);
        self.canvas_set_pixel(local_x, local_y, colour);
    }

    fn canvas_fill_rect_at_screen_rect(&mut self, rect: Rect, colour: u8) {
        for yy in 0..rect.h {
            for xx in 0..rect.w {
                self.canvas_set_pixel_at_screen_point(
                    rect.x.saturating_add(i32::from(xx)),
                    rect.y.saturating_add(i32::from(yy)),
                    colour,
                );
            }
        }
    }

    fn canvas_stroke_rect_at_screen_rect(&mut self, rect: Rect, colour: u8, width: u16) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        for inset in 0..width.min(rect.w).min(rect.h) {
            let x = rect.x.saturating_add(i32::from(inset));
            let y = rect.y.saturating_add(i32::from(inset));
            let w = rect.w.saturating_sub(inset.saturating_mul(2));
            let h = rect.h.saturating_sub(inset.saturating_mul(2));
            if w == 0 || h == 0 {
                continue;
            }
            let right = x.saturating_add(i32::from(w.saturating_sub(1)));
            let bottom = y.saturating_add(i32::from(h.saturating_sub(1)));
            for xx in 0..w {
                self.canvas_set_pixel_at_screen_point(x.saturating_add(i32::from(xx)), y, colour);
                self.canvas_set_pixel_at_screen_point(
                    x.saturating_add(i32::from(xx)),
                    bottom,
                    colour,
                );
            }
            for yy in 0..h {
                self.canvas_set_pixel_at_screen_point(
                    right,
                    y.saturating_add(i32::from(yy)),
                    colour,
                );
                self.canvas_set_pixel_at_screen_point(x, y.saturating_add(i32::from(yy)), colour);
            }
        }
    }

    fn canvas_line_screen(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, colour: u8, width: u16) {
        let mut x0 = x0;
        let mut y0 = y0;
        let dx = x1.saturating_sub(x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -y1.saturating_sub(y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx.saturating_add(dy);
        loop {
            self.canvas_thick_point_screen(x0, y0, width, colour);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = err.saturating_mul(2);
            if e2 >= dy {
                err = err.saturating_add(dy);
                x0 = x0.saturating_add(sx);
            }
            if e2 <= dx {
                err = err.saturating_add(dx);
                y0 = y0.saturating_add(sy);
            }
        }
    }

    fn canvas_ellipse_screen(&mut self, rect: Rect, colour: u8, width: u16) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let w_i = i64::from(rect.w);
        let h_i = i64::from(rect.h);
        let threshold = w_i * w_i * h_i * h_i;
        let cx2 = i64::from(rect.x)
            .saturating_mul(2)
            .saturating_add(i64::from(rect.w.saturating_sub(1)));
        let cy2 = i64::from(rect.y)
            .saturating_mul(2)
            .saturating_add(i64::from(rect.h.saturating_sub(1)));
        for yy in 0..rect.h {
            for xx in 0..rect.w {
                let px = rect.x.saturating_add(i32::from(xx));
                let py = rect.y.saturating_add(i32::from(yy));
                if ellipse_contains(px, py, cx2, cy2, w_i, h_i, threshold)
                    && (!ellipse_contains(px - 1, py, cx2, cy2, w_i, h_i, threshold)
                        || !ellipse_contains(
                            px.saturating_add(1),
                            py,
                            cx2,
                            cy2,
                            w_i,
                            h_i,
                            threshold,
                        )
                        || !ellipse_contains(px, py - 1, cx2, cy2, w_i, h_i, threshold)
                        || !ellipse_contains(
                            px,
                            py.saturating_add(1),
                            cx2,
                            cy2,
                            w_i,
                            h_i,
                            threshold,
                        ))
                {
                    self.canvas_thick_point_screen(px, py, width, colour);
                }
            }
        }
    }

    fn canvas_fill_ellipse_screen(&mut self, rect: Rect, colour: u8) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let w_i = i64::from(rect.w);
        let h_i = i64::from(rect.h);
        let threshold = w_i * w_i * h_i * h_i;
        let cx2 = i64::from(rect.x)
            .saturating_mul(2)
            .saturating_add(i64::from(rect.w.saturating_sub(1)));
        let cy2 = i64::from(rect.y)
            .saturating_mul(2)
            .saturating_add(i64::from(rect.h.saturating_sub(1)));
        for yy in 0..rect.h {
            for xx in 0..rect.w {
                let px = rect.x.saturating_add(i32::from(xx));
                let py = rect.y.saturating_add(i32::from(yy));
                if ellipse_contains(px, py, cx2, cy2, w_i, h_i, threshold) {
                    self.canvas_set_pixel_at_screen_point(px, py, colour);
                }
            }
        }
    }

    fn canvas_polygon_screen(&mut self, points: &[(i32, i32)], colour: u8, width: u16) {
        for pair in points.windows(2) {
            self.canvas_line_screen(pair[0].0, pair[0].1, pair[1].0, pair[1].1, colour, width);
        }
    }

    fn canvas_fill_polygon_screen(&mut self, points: &[(i32, i32)], colour: u8) {
        if points.len() < 3 {
            return;
        }
        let (mut left, mut top, mut right, mut bottom) =
            (points[0].0, points[0].1, points[0].0, points[0].1);
        for &(x, y) in points.iter().skip(1) {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
        for y in top..=bottom {
            for x in left..=right {
                if point_in_polygon(x, y, points) {
                    self.canvas_set_pixel_at_screen_point(x, y, colour);
                }
            }
        }
    }

    fn canvas_draw_text_screen(
        &mut self,
        rect: Rect,
        foreground: u8,
        background: u8,
        transparent: bool,
        layout: &TextLayout,
    ) {
        if !transparent {
            self.canvas_fill_rect_at_screen_rect(rect, background);
        }
        for line in &layout.lines {
            for (col, ch) in line.text.chars().enumerate() {
                if ch.is_whitespace() {
                    continue;
                }
                let x = rect
                    .x
                    .saturating_add(line.x_offset)
                    .saturating_add(i32::try_from(col).unwrap_or(i32::MAX).saturating_mul(6));
                let y = rect.y.saturating_add(line.y_offset);
                self.canvas_fill_rect_at_screen_rect(Rect::new(x, y, 5, 7), foreground);
            }
        }
    }

    fn copy_pixels_for_picture(
        &self,
        request: FramebufferCopyRequest,
    ) -> Option<(u16, u16, Vec<u8>)> {
        if self.canvas_pixels.is_empty() {
            return None;
        }
        let (source_x, source_y, width, height, zoom) = match request.source {
            GraphicsContextCopySource::Canvas => (0, 0, self.canvas_width, self.canvas_height, 1.0),
            GraphicsContextCopySource::Viewport => (
                request.viewport.x.saturating_sub(self.canvas.x),
                request.viewport.y.saturating_sub(self.canvas.y),
                request.viewport.w,
                request.viewport.h,
                framebuffer_copy_zoom(request.zoom_raw.or(self.zoom_raw)),
            ),
        };
        if width == 0 || height == 0 {
            return None;
        }
        let len = usize::from(width).checked_mul(usize::from(height))?;
        if len > MAX_FRAMEBUFFER_GRAPHICS_CONTEXT_PIXELS {
            return None;
        }
        let mut data = Vec::with_capacity(len);
        for yy in 0..height {
            for xx in 0..width {
                let local_x = source_x.saturating_add(zoomed_copy_offset(xx, zoom));
                let local_y = source_y.saturating_add(zoomed_copy_offset(yy, zoom));
                data.push(self.canvas_pixel_or_transparency(local_x, local_y));
            }
        }
        Some((width, height, data))
    }

    fn canvas_thick_point_screen(&mut self, x: i32, y: i32, width: u16, colour: u8) {
        if width == 0 {
            return;
        }
        let before = i32::from(width.saturating_sub(1) / 2);
        let after = i32::from(width / 2);
        for yy in -before..=after {
            for xx in -before..=after {
                self.canvas_set_pixel_at_screen_point(
                    x.saturating_add(xx),
                    y.saturating_add(yy),
                    colour,
                );
            }
        }
    }

    fn canvas_set_pixel(&mut self, x: i32, y: i32, colour: u8) {
        if x < 0 || y < 0 {
            return;
        }
        let Ok(x) = u16::try_from(x) else {
            return;
        };
        let Ok(y) = u16::try_from(y) else {
            return;
        };
        if x >= self.canvas_width || y >= self.canvas_height {
            return;
        }
        let index = usize::from(y)
            .saturating_mul(usize::from(self.canvas_width))
            .saturating_add(usize::from(x));
        if let Some(pixel) = self.canvas_pixels.get_mut(index) {
            *pixel = colour;
        }
    }

    fn canvas_pixel_or_transparency(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 {
            return self.transparency_colour;
        }
        let x = u16::try_from(x).ok();
        let y = u16::try_from(y).ok();
        match (x, y) {
            (Some(x), Some(y)) if x < self.canvas_width && y < self.canvas_height => {
                let index = usize::from(y)
                    .saturating_mul(usize::from(self.canvas_width))
                    .saturating_add(usize::from(x));
                self.canvas_pixels
                    .get(index)
                    .copied()
                    .unwrap_or(self.transparency_colour)
            }
            _ => self.transparency_colour,
        }
    }
}

fn graphics_context_canvas_pixels(width: u16, height: u16, initial: u8) -> Vec<u8> {
    let Some(len) = usize::from(width).checked_mul(usize::from(height)) else {
        return Vec::new();
    };
    if len > MAX_FRAMEBUFFER_GRAPHICS_CONTEXT_PIXELS {
        return Vec::new();
    }
    vec![initial; len]
}

fn graphics_context_state_for(
    contexts: &mut Vec<FramebufferGraphicsContextState>,
    object_id: ObjectID,
    viewport: Rect,
) -> &mut FramebufferGraphicsContextState {
    if let Some(index) = contexts
        .iter()
        .position(|state| state.object_id == object_id)
    {
        return &mut contexts[index];
    }
    contexts.push(FramebufferGraphicsContextState::new(object_id, viewport));
    contexts.last_mut().expect("state was just pushed")
}

struct FramebufferGraphicsContextReplayTarget<'a> {
    fb: &'a mut Framebuffer,
    contexts: &'a mut [FramebufferGraphicsContextState],
    picture_updates: &'a mut Vec<PictureUpdate>,
    palette: &'a Palette,
    clip: Rect,
}

fn apply_graphics_context_replay(
    target: FramebufferGraphicsContextReplayTarget<'_>,
    object_id: ObjectID,
    subcommand: u8,
    payload: &[u8],
) {
    let Some(state) = target
        .contexts
        .iter_mut()
        .find(|state| state.object_id == object_id)
    else {
        return;
    };
    let clip = intersect_rect(target.clip, state.viewport);
    match subcommand {
        0x00 => {
            let Some((x, y)) = decode_graphics_context_i16_pair(payload) else {
                return;
            };
            state.cursor_x = x;
            state.cursor_y = y;
        }
        0x01 => {
            let Some((dx, dy)) = decode_graphics_context_i16_pair(payload) else {
                return;
            };
            state.cursor_x = state.cursor_x.saturating_add(dx);
            state.cursor_y = state.cursor_y.saturating_add(dy);
        }
        0x02 => {
            if let Some(index) = payload.first().copied() {
                state.foreground = index;
            }
        }
        0x03 => {
            if let Some(index) = payload.first().copied() {
                state.background = index;
            }
        }
        0x04 => {
            let Some(line_attributes_id) = decode_graphics_context_object_id(payload) else {
                return;
            };
            state.line_enabled = line_attributes_id != ObjectID::NULL;
        }
        0x05 => {
            let Some(fill_attributes_id) = decode_graphics_context_object_id(payload) else {
                return;
            };
            state.fill_enabled = fill_attributes_id != ObjectID::NULL;
        }
        0x06 => {
            let Some(font_attributes_id) = decode_graphics_context_object_id(payload) else {
                return;
            };
            state.font_attributes_selected = true;
            if font_attributes_id == ObjectID::NULL {
                state.text_foreground = 1;
                state.font = FontMetrics::default();
                state.decoration = FontDecoration::default();
            } else {
                // A bare framebuffer replay stream does not carry the
                // referenced FontAttributes object body. Preserve deterministic
                // best-effort behaviour by freezing the current foreground as
                // the text colour until a richer command stream supplies the
                // expanded text commands or another font selector arrives.
                state.text_foreground = state.foreground;
            }
        }
        0x07 => {
            let Some((w, h)) = decode_graphics_context_u16_pair(payload) else {
                return;
            };
            let rect = state.rect_at_cursor(w, h);
            state.canvas_fill_rect_at_screen_rect(rect, state.background);
            fill_rect(
                target.fb,
                rect,
                target.palette.resolve(state.background),
                clip,
            );
            state.move_to_bottom_right_inside(w, h);
        }
        0x08 => {
            let Some((dx, dy)) = decode_graphics_context_i16_pair(payload) else {
                return;
            };
            state.cursor_x = state.cursor_x.saturating_add(dx);
            state.cursor_y = state.cursor_y.saturating_add(dy);
            let (x, y) = state.device_point();
            state.canvas_set_pixel_at_screen_point(x, y, state.foreground);
            target
                .fb
                .set_pixel(x, y, target.palette.resolve(state.foreground), clip);
        }
        0x09 => {
            let Some((dx, dy)) = decode_graphics_context_i16_pair(payload) else {
                return;
            };
            let (x0, y0) = state.device_point();
            state.cursor_x = state.cursor_x.saturating_add(dx);
            state.cursor_y = state.cursor_y.saturating_add(dy);
            let (x1, y1) = state.device_point();
            if state.line_enabled {
                state.canvas_line_screen(x0, y0, x1, y1, state.foreground, state.line_width);
                draw_line(
                    target.fb,
                    LineStroke {
                        x0,
                        y0,
                        x1,
                        y1,
                        colour: target.palette.resolve(state.foreground),
                        width: state.line_width,
                        line_art: 0xFFFF,
                    },
                    clip,
                );
            }
        }
        0x0A => {
            let Some((w, h)) = decode_graphics_context_u16_pair(payload) else {
                return;
            };
            let rect = state.rect_at_cursor(w, h);
            if state.fill_enabled {
                state.canvas_fill_rect_at_screen_rect(rect, state.background);
                fill_rect(
                    target.fb,
                    rect,
                    target.palette.resolve(state.background),
                    clip,
                );
            }
            if state.line_enabled {
                state.canvas_stroke_rect_at_screen_rect(rect, state.foreground, state.line_width);
                stroke_rect(
                    target.fb,
                    rect,
                    target.palette.resolve(state.foreground),
                    state.line_width,
                    0xFFFF,
                    0,
                    clip,
                );
            }
            state.move_to_bottom_right_inside(w, h);
        }
        0x0B => {
            let Some((w, h)) = decode_graphics_context_u16_pair(payload) else {
                return;
            };
            let rect = state.rect_at_cursor(w, h);
            if state.fill_enabled {
                state.canvas_fill_ellipse_screen(rect, state.background);
            }
            if state.line_enabled {
                state.canvas_ellipse_screen(rect, state.foreground, state.line_width);
            }
            if state.fill_enabled || state.line_enabled {
                draw_ellipse(
                    target.fb,
                    rect,
                    ShapeDrawStyle {
                        line_colour: target.palette.resolve(state.foreground),
                        fill_colour: target.palette.resolve(state.background),
                        filled: state.fill_enabled,
                        line_enabled: state.line_enabled,
                        line_width: state.line_width,
                        line_art: 0xFFFF,
                    },
                    clip,
                );
            }
            state.move_to_bottom_right_inside(w, h);
        }
        0x0C => {
            let Some(points) = decode_graphics_context_polygon_points(payload) else {
                return;
            };
            if points.is_empty() {
                return;
            }
            let origin = state.device_point();
            let start_cursor_x = state.cursor_x;
            let start_cursor_y = state.cursor_y;
            let mut absolute_points = Vec::with_capacity(points.len().saturating_add(1));
            absolute_points.push(origin);
            for (dx, dy) in points {
                absolute_points.push((origin.0.saturating_add(dx), origin.1.saturating_add(dy)));
                state.cursor_x = start_cursor_x.saturating_add(dx);
                state.cursor_y = start_cursor_y.saturating_add(dy);
            }
            let filled =
                state.fill_enabled && absolute_points.last().is_some_and(|last| *last == origin);
            if filled {
                state.canvas_fill_polygon_screen(&absolute_points, state.background);
            }
            if state.line_enabled {
                state.canvas_polygon_screen(&absolute_points, state.foreground, state.line_width);
            }
            if filled || state.line_enabled {
                draw_polygon(
                    target.fb,
                    &absolute_points,
                    ShapeDrawStyle {
                        line_colour: target.palette.resolve(state.foreground),
                        fill_colour: target.palette.resolve(state.background),
                        filled,
                        line_enabled: state.line_enabled,
                        line_width: state.line_width,
                        line_art: 0xFFFF,
                    },
                    clip,
                );
            }
        }
        0x0D => {
            let Some((transparent, text)) = decode_graphics_context_draw_text(payload) else {
                return;
            };
            let text_foreground = state.resolved_text_foreground();
            let text_rect = state.rect_at_cursor(state.viewport.w, state.viewport.h);
            let layout = text::layout_text(
                &text,
                state.font,
                text_rect.w,
                text_rect.h,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            );
            if !transparent {
                fill_rect(
                    target.fb,
                    text_rect,
                    target.palette.resolve(state.background),
                    clip,
                );
            }
            state.canvas_draw_text_screen(
                text_rect,
                text_foreground,
                state.background,
                transparent,
                &layout,
            );
            draw_text_cells(
                target.fb,
                text_rect,
                TextCellStyle {
                    foreground: target.palette.resolve(text_foreground),
                    background: target.palette.resolve(state.background),
                    metrics: state.font,
                    decoration: state.decoration,
                },
                &layout,
                clip,
            );
            let cursor_advance_x = layout
                .lines
                .iter()
                .map(|line| line.text.chars().count())
                .max()
                .unwrap_or(0)
                .saturating_mul(usize::from(FontMetrics::default().cell_w))
                .saturating_sub(1);
            let cursor_advance_y = layout
                .lines
                .len()
                .saturating_mul(usize::from(FontMetrics::default().cell_h))
                .saturating_sub(1);
            state.cursor_x = state
                .cursor_x
                .saturating_add(usize_to_i32(cursor_advance_x));
            state.cursor_y = state
                .cursor_y
                .saturating_add(usize_to_i32(cursor_advance_y));
        }
        0x0E => {
            let Some((x, y)) = decode_graphics_context_i16_pair(payload) else {
                return;
            };
            state.set_viewport_position(x, y);
        }
        0x0F => {
            let Some(zoom_raw) = decode_graphics_context_u32(payload) else {
                return;
            };
            state.zoom_raw = Some(zoom_raw);
        }
        0x10 => {
            if payload.len() < 8 {
                return;
            }
            let Some((x, y)) = decode_graphics_context_i16_pair(&payload[..4]) else {
                return;
            };
            let Some(zoom_raw) = decode_graphics_context_u32(&payload[4..]) else {
                return;
            };
            state.set_viewport_position(x, y);
            state.zoom_raw = Some(zoom_raw);
        }
        0x11 => {
            let Some((w, h)) = decode_graphics_context_u16_pair(payload) else {
                return;
            };
            state.set_viewport_size(w, h);
        }
        0x13 | 0x14 => {
            let Some(picture_id) = decode_graphics_context_object_id(payload) else {
                return;
            };
            if picture_id == ObjectID::NULL {
                return;
            }
            let source = if subcommand == 0x13 {
                GraphicsContextCopySource::Canvas
            } else {
                GraphicsContextCopySource::Viewport
            };
            let request = FramebufferCopyRequest {
                object_id,
                picture_id,
                source,
                viewport: state.viewport,
                zoom_raw: state.zoom_raw,
            };
            if let Some((width, height, data)) = state.copy_pixels_for_picture(request) {
                record_picture_pixels(
                    target.picture_updates,
                    picture_id,
                    width,
                    height,
                    2,
                    Some(state.transparency_colour),
                    data,
                );
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone)]
struct PictureUpdate {
    picture_id: ObjectID,
    width: u16,
    height: u16,
    format: u8,
    transparent_index: Option<u8>,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct FramebufferCopyRequest {
    object_id: ObjectID,
    picture_id: ObjectID,
    source: GraphicsContextCopySource,
    viewport: Rect,
    zoom_raw: Option<u32>,
}

fn record_picture_update(command: &RenderCommand, updates: &mut Vec<PictureUpdate>) {
    if let RenderCommand::GraphicsContextPictureData {
        picture_id,
        width,
        height,
        format,
        transparent_index,
        data,
        ..
    } = command
    {
        if let Some(existing) = updates
            .iter_mut()
            .find(|update| update.picture_id == *picture_id)
        {
            existing.width = *width;
            existing.height = *height;
            existing.format = *format;
            existing.transparent_index = Some(*transparent_index);
            existing.data.clear();
            existing.data.extend_from_slice(data);
        } else {
            updates.push(PictureUpdate {
                picture_id: *picture_id,
                width: *width,
                height: *height,
                format: *format,
                transparent_index: Some(*transparent_index),
                data: data.clone(),
            });
        }
    }
}

fn picture_update_for(updates: &[PictureUpdate], object_id: ObjectID) -> Option<&PictureUpdate> {
    updates
        .iter()
        .rev()
        .find(|update| update.picture_id == object_id)
}

fn record_framebuffer_copy_to_picture(
    contexts: &[FramebufferGraphicsContextState],
    request: FramebufferCopyRequest,
    updates: &mut Vec<PictureUpdate>,
) {
    let Some(state) = contexts
        .iter()
        .find(|state| state.object_id == request.object_id)
    else {
        return;
    };
    if let Some((width, height, data)) = state.copy_pixels_for_picture(request) {
        record_picture_pixels(
            updates,
            request.picture_id,
            width,
            height,
            2,
            Some(state.transparency_colour),
            data,
        );
    }
}

fn framebuffer_copy_zoom(zoom_raw: Option<u32>) -> f32 {
    zoom_raw
        .map(f32::from_bits)
        .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
        .unwrap_or(1.0)
}

fn zoomed_copy_offset(offset: u16, zoom: f32) -> i32 {
    (f32::from(offset) / zoom).floor() as i32
}

fn record_picture_pixels(
    updates: &mut Vec<PictureUpdate>,
    picture_id: ObjectID,
    width: u16,
    height: u16,
    format: u8,
    transparent_index: Option<u8>,
    data: Vec<u8>,
) {
    if let Some(existing) = updates
        .iter_mut()
        .find(|update| update.picture_id == picture_id)
    {
        existing.width = width;
        existing.height = height;
        existing.format = format;
        existing.transparent_index = transparent_index;
        existing.data = data;
    } else {
        updates.push(PictureUpdate {
            picture_id,
            width,
            height,
            format,
            transparent_index,
            data,
        });
    }
}

fn decode_graphics_context_i16_pair(payload: &[u8]) -> Option<(i32, i32)> {
    if payload.len() < 4 {
        return None;
    }
    Some((
        i32::from(i16::from_le_bytes([payload[0], payload[1]])),
        i32::from(i16::from_le_bytes([payload[2], payload[3]])),
    ))
}

fn decode_graphics_context_u16_pair(payload: &[u8]) -> Option<(u16, u16)> {
    if payload.len() < 4 {
        return None;
    }
    Some((
        u16::from_le_bytes([payload[0], payload[1]]),
        u16::from_le_bytes([payload[2], payload[3]]),
    ))
}

fn decode_graphics_context_u32(payload: &[u8]) -> Option<u32> {
    if payload.len() < 4 {
        return None;
    }
    Some(u32::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ]))
}

fn decode_graphics_context_object_id(payload: &[u8]) -> Option<ObjectID> {
    if payload.len() < 2 {
        return None;
    }
    Some(ObjectID::new(u16::from_le_bytes([payload[0], payload[1]])))
}

fn decode_graphics_context_polygon_points(payload: &[u8]) -> Option<Vec<(i32, i32)>> {
    let count = usize::from(*payload.first()?);
    let bytes = count.checked_mul(4)?.checked_add(1)?;
    if payload.len() != bytes {
        return None;
    }
    let mut points = Vec::with_capacity(count);
    for chunk in payload[1..].chunks_exact(4) {
        points.push((
            i32::from(i16::from_le_bytes([chunk[0], chunk[1]])),
            i32::from(i16::from_le_bytes([chunk[2], chunk[3]])),
        ));
    }
    Some(points)
}

fn decode_graphics_context_draw_text(payload: &[u8]) -> Option<(bool, String)> {
    if payload.len() < 2 {
        return None;
    }
    let transparent = payload[0] != 0;
    let len = usize::from(payload[1]);
    let end = 2usize.checked_add(len)?;
    if end > payload.len() {
        return None;
    }
    Some((
        transparent,
        String::from_utf8_lossy(&payload[2..end]).into_owned(),
    ))
}

fn usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn framebuffer_clip(rect: Rect, width: u16, height: u16) -> Rect {
    let left = rect.x.max(0);
    let top = rect.y.max(0);
    let right = rect.right().min(i32::from(width)).max(left);
    let bottom = rect.bottom().min(i32::from(height)).max(top);
    Rect::new(
        left,
        top,
        u16::try_from(right - left).unwrap_or(0),
        u16::try_from(bottom - top).unwrap_or(0),
    )
}

fn intersect_rect(a: Rect, b: Rect) -> Rect {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = a.right().min(b.right()).max(left);
    let bottom = a.bottom().min(b.bottom()).max(top);
    Rect::new(
        left,
        top,
        u16::try_from(right - left).unwrap_or(0),
        u16::try_from(bottom - top).unwrap_or(0),
    )
}

fn command_bounds(commands: &[RenderCommand]) -> Option<(u16, u16)> {
    let mut right = 0i32;
    let mut bottom = 0i32;
    for command in commands {
        match command {
            RenderCommand::FillRect { rect, .. }
            | RenderCommand::StrokeRect { rect, .. }
            | RenderCommand::DrawText { rect, .. }
            | RenderCommand::IndexedImage { rect, .. }
            | RenderCommand::RgbaImage { rect, .. }
            | RenderCommand::Placeholder { rect, .. }
            | RenderCommand::SoftKey { rect, .. }
            | RenderCommand::GraphicsContextCanvas { rect, .. }
            | RenderCommand::Clip(rect) => {
                right = right.max(rect.right());
                bottom = bottom.max(rect.bottom());
            }
            RenderCommand::Line { x0, y0, x1, y1, .. } => {
                right = right.max((*x0).max(*x1).saturating_add(1));
                bottom = bottom.max((*y0).max(*y1).saturating_add(1));
            }
            RenderCommand::Ellipse { rect, .. }
            | RenderCommand::EllipseArc { rect, .. }
            | RenderCommand::PatternFillRect { rect, .. }
            | RenderCommand::PatternFillEllipse { rect, .. }
            | RenderCommand::Meter { rect, .. }
            | RenderCommand::BarGraph { rect, .. } => {
                right = right.max(rect.right());
                bottom = bottom.max(rect.bottom());
            }
            RenderCommand::Polygon { points, .. }
            | RenderCommand::PatternFillPolygon { points, .. } => {
                for &(x, y) in points {
                    right = right.max(x.saturating_add(1));
                    bottom = bottom.max(y.saturating_add(1));
                }
            }
            RenderCommand::GraphicsContextViewport { viewport, .. }
            | RenderCommand::GraphicsContextCopyToPicture { viewport, .. } => {
                right = right.max(viewport.right());
                bottom = bottom.max(viewport.bottom());
            }
            RenderCommand::GraphicsContextPictureData { .. }
            | RenderCommand::GraphicsContextReplay { .. } => {}
        }
    }
    if right <= 0 || bottom <= 0 {
        return None;
    }
    Some((u16::try_from(right).ok()?, u16::try_from(bottom).ok()?))
}

fn fill_rect(fb: &mut Framebuffer, rect: Rect, colour: Colour, clip: Rect) {
    for yy in 0..rect.h {
        for xx in 0..rect.w {
            fb.set_pixel(
                rect.x.saturating_add(i32::from(xx)),
                rect.y.saturating_add(i32::from(yy)),
                colour,
                clip,
            );
        }
    }
}
