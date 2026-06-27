fn graphics_context_render_commands(
    pool: &ObjectPool,
    scene: &Scene,
    engine: &LayoutEngine,
    renderer: &GtuiRenderer,
    command: &GraphicsContextCommand,
    background_override: Option<u8>,
    states: &mut HashMap<ObjectID, GraphicsContextDrawState>,
) -> Vec<RenderCommand> {
    let Some(canvas_viewport_base) = graphic_context_viewport(pool, command.object_id) else {
        return Vec::new();
    };
    let device_viewport_base = scene
        .find(command.object_id)
        .map(|node| node.rect)
        .unwrap_or(canvas_viewport_base);
    let placement_dx = device_viewport_base
        .x
        .saturating_sub(canvas_viewport_base.x);
    let placement_dy = device_viewport_base
        .y
        .saturating_sub(canvas_viewport_base.y);
    let state = states.entry(command.object_id).or_insert_with(|| {
        GraphicsContextDrawState::new(pool, command.object_id, renderer, background_override)
    });
    let canvas_viewport = state.effective_viewport(canvas_viewport_base);
    let viewport = canvas_viewport.translate(placement_dx, placement_dy);

    match command.subcommand {
        0x00 => {
            let Some((x, y)) = decode_graphics_context_i16_pair(&command.payload) else {
                return Vec::new();
            };
            state.cursor_x = x;
            state.cursor_y = y;
            Vec::new()
        }
        0x01 => {
            let Some((dx, dy)) = decode_graphics_context_i16_pair(&command.payload) else {
                return Vec::new();
            };
            state.cursor_x = state.cursor_x.saturating_add(dx);
            state.cursor_y = state.cursor_y.saturating_add(dy);
            Vec::new()
        }
        0x02 => {
            let Some(colour_index) = command.payload.first().copied() else {
                return Vec::new();
            };
            state.foreground_index = colour_index;
            state.foreground = renderer.palette().resolve(colour_index);
            Vec::new()
        }
        0x03 => {
            let Some(colour_index) = command.payload.first().copied() else {
                return Vec::new();
            };
            state.background_index = colour_index;
            state.background = renderer.palette().resolve(colour_index);
            Vec::new()
        }
        0x04 => {
            let Some(id) = decode_graphics_context_object_id(&command.payload) else {
                return Vec::new();
            };
            state.use_attribute_colours = true;
            state.apply_line_attributes(pool, renderer, id);
            Vec::new()
        }
        0x05 => {
            let Some(id) = decode_graphics_context_object_id(&command.payload) else {
                return Vec::new();
            };
            state.use_attribute_colours = true;
            state.apply_fill_attributes(pool, renderer, id);
            Vec::new()
        }
        0x06 => {
            let Some(id) = decode_graphics_context_object_id(&command.payload) else {
                return Vec::new();
            };
            state.use_attribute_colours = true;
            state.apply_font_attributes(pool, renderer, id);
            Vec::new()
        }
        0x07 => {
            let Some((w, h)) = decode_graphics_context_u16_pair(&command.payload) else {
                return Vec::new();
            };
            let draw_rect = state.rect_at_cursor(viewport, w, h);
            state.fill_canvas_rect_at_cursor(w, h, state.background_index);
            state.move_to_bottom_right_inside(w, h);
            vec![RenderCommand::FillRect {
                rect: draw_rect,
                colour: state.background,
            }]
        }
        0x08 => {
            let Some((dx, dy)) = decode_graphics_context_i16_pair(&command.payload) else {
                return Vec::new();
            };
            state.cursor_x = state.cursor_x.saturating_add(dx);
            state.cursor_y = state.cursor_y.saturating_add(dy);
            state.canvas_set_current_pixel(state.foreground_index);
            vec![RenderCommand::FillRect {
                rect: state.rect_at_cursor(viewport, 1, 1),
                colour: state.foreground,
            }]
        }
        0x09 => {
            let Some((dx, dy)) = decode_graphics_context_i16_pair(&command.payload) else {
                return Vec::new();
            };
            let (x0, y0) = state.device_point(viewport);
            let start_cursor_x = state.cursor_x;
            let start_cursor_y = state.cursor_y;
            state.cursor_x = state.cursor_x.saturating_add(dx);
            state.cursor_y = state.cursor_y.saturating_add(dy);
            let (x1, y1) = state.device_point(viewport);
            if !state.line_enabled {
                return Vec::new();
            }
            state.canvas_line(
                (start_cursor_x, start_cursor_y),
                (state.cursor_x, state.cursor_y),
                state.foreground_index,
                state.line_width,
                state.line_art,
            );
            vec![RenderCommand::Line {
                x0,
                y0,
                x1,
                y1,
                colour: state.foreground,
                width: state.line_width,
                line_art: state.line_art,
            }]
        }
        0x0A => {
            let Some((w, h)) = decode_graphics_context_u16_pair(&command.payload) else {
                return Vec::new();
            };
            let draw_rect = state.rect_at_cursor(viewport, w, h);
            if state.is_shape_filled() {
                state.fill_canvas_rect_at_cursor(w, h, state.effective_fill_index());
            }
            if state.line_enabled {
                state.stroke_canvas_rect_at_cursor(
                    w,
                    h,
                    state.foreground_index,
                    state.line_width,
                    state.line_art,
                );
            }
            state.move_to_bottom_right_inside(w, h);
            let mut commands = Vec::new();
            if state.is_shape_filled() {
                commands.push(RenderCommand::FillRect {
                    rect: draw_rect,
                    colour: state.effective_fill_colour(),
                });
            }
            if state.line_enabled {
                commands.push(RenderCommand::StrokeRect {
                    rect: draw_rect,
                    colour: state.foreground,
                    width: state.line_width,
                    line_art: state.line_art,
                    suppress: 0,
                });
            }
            commands
        }
        0x0B => {
            let Some((w, h)) = decode_graphics_context_u16_pair(&command.payload) else {
                return Vec::new();
            };
            let draw_rect = state.rect_at_cursor(viewport, w, h);
            let filled = state.is_shape_filled();
            state.canvas_ellipse_at_cursor(
                w,
                h,
                CanvasDrawStyle {
                    filled,
                    fill_colour: state.effective_fill_index(),
                    line_colour: state.foreground_index,
                    line_enabled: state.line_enabled,
                    line_width: state.line_width,
                    line_art: state.line_art,
                },
            );
            state.move_to_bottom_right_inside(w, h);
            if !filled && !state.line_enabled {
                return Vec::new();
            }
            vec![RenderCommand::Ellipse {
                rect: draw_rect,
                colour: state.foreground,
                fill_colour: state.effective_fill_colour(),
                filled,
                width: if state.line_enabled {
                    state.line_width
                } else {
                    0
                },
                line_art: state.line_art,
            }]
        }
        0x0C => {
            let Some(points) = decode_graphics_context_polygon_points(&command.payload) else {
                return Vec::new();
            };
            let origin = state.device_point(viewport);
            let start_cursor_x = state.cursor_x;
            let start_cursor_y = state.cursor_y;
            let mut absolute_points = Vec::with_capacity(points.len().saturating_add(1));
            let mut canvas_points = Vec::with_capacity(points.len().saturating_add(1));
            absolute_points.push(origin);
            canvas_points.push((start_cursor_x, start_cursor_y));
            for (dx, dy) in points {
                absolute_points.push((origin.0.saturating_add(dx), origin.1.saturating_add(dy)));
                canvas_points.push((
                    start_cursor_x.saturating_add(dx),
                    start_cursor_y.saturating_add(dy),
                ));
                state.cursor_x = start_cursor_x.saturating_add(dx);
                state.cursor_y = start_cursor_y.saturating_add(dy);
            }
            let filled = absolute_points.last().is_some_and(|last| *last == origin)
                && state.is_shape_filled();
            if !filled && !state.line_enabled {
                return Vec::new();
            }
            state.canvas_polygon(
                &canvas_points,
                CanvasDrawStyle {
                    filled,
                    fill_colour: state.effective_fill_index(),
                    line_colour: state.foreground_index,
                    line_enabled: state.line_enabled,
                    line_width: state.line_width,
                    line_art: state.line_art,
                },
            );
            vec![RenderCommand::Polygon {
                origin,
                points: absolute_points,
                colour: state.foreground,
                fill_colour: state.effective_fill_colour(),
                filled,
                width: if state.line_enabled {
                    state.line_width
                } else {
                    0
                },
                line_art: state.line_art,
            }]
        }
        0x0D => {
            let Some((transparent, text)) = decode_graphics_context_draw_text(&command.payload)
            else {
                return Vec::new();
            };

            let style = state.text_style();
            let text_rect = state.rect_at_cursor(viewport, viewport.w, viewport.h);
            let layout = text_layout::layout_text(
                &text,
                style.font,
                text_rect.w,
                text_rect.h,
                HorizontalAlign::Left,
                VerticalAlign::Top,
                false,
            );
            state.canvas_draw_text(text_rect.w, text_rect.h, transparent, &layout);
            let mut commands = Vec::new();
            if !transparent {
                commands.push(RenderCommand::FillRect {
                    rect: text_rect,
                    colour: style.background,
                });
            }
            let cursor_advance_x = layout
                .lines
                .iter()
                .map(|line| line.text.chars().count())
                .max()
                .unwrap_or(0)
                .saturating_mul(usize::from(style.font.cell_w))
                .saturating_sub(1);
            let cursor_advance_y = layout
                .lines
                .len()
                .saturating_mul(usize::from(style.font.cell_h))
                .saturating_sub(1);
            state.cursor_x = state
                .cursor_x
                .saturating_add(usize_to_i32(cursor_advance_x));
            state.cursor_y = state
                .cursor_y
                .saturating_add(usize_to_i32(cursor_advance_y));
            commands.push(RenderCommand::DrawText {
                rect: text_rect,
                text: layout.rendered(),
                style,
                align: HorizontalAlign::Left,
                layout,
            });
            commands
        }
        0x0E => {
            let Some((x, y)) = decode_graphics_context_i16_pair(&command.payload) else {
                return Vec::new();
            };
            state.set_viewport_position(canvas_viewport_base, x, y);
            let viewport = state
                .effective_viewport(canvas_viewport_base)
                .translate(placement_dx, placement_dy);
            vec![RenderCommand::GraphicsContextViewport {
                object_id: command.object_id,
                viewport,
                zoom_raw: state.zoom_raw,
            }]
        }
        0x0F => {
            let Some(zoom_raw) = decode_graphics_context_u32(&command.payload) else {
                return Vec::new();
            };
            state.zoom_raw = Some(zoom_raw);
            let viewport = state
                .effective_viewport(canvas_viewport_base)
                .translate(placement_dx, placement_dy);
            vec![RenderCommand::GraphicsContextViewport {
                object_id: command.object_id,
                viewport,
                zoom_raw: state.zoom_raw,
            }]
        }
        0x10 => {
            if command.payload.len() < 8 {
                return Vec::new();
            }
            let Some((x, y)) = decode_graphics_context_i16_pair(&command.payload[..4]) else {
                return Vec::new();
            };
            let Some(zoom_raw) = decode_graphics_context_u32(&command.payload[4..]) else {
                return Vec::new();
            };
            state.set_viewport_position(canvas_viewport_base, x, y);
            state.zoom_raw = Some(zoom_raw);
            let viewport = state
                .effective_viewport(canvas_viewport_base)
                .translate(placement_dx, placement_dy);
            vec![RenderCommand::GraphicsContextViewport {
                object_id: command.object_id,
                viewport,
                zoom_raw: state.zoom_raw,
            }]
        }
        0x11 => {
            let Some((w, h)) = decode_graphics_context_u16_pair(&command.payload) else {
                return Vec::new();
            };
            state.set_viewport_size(canvas_viewport_base, w, h);
            let viewport = state
                .effective_viewport(canvas_viewport_base)
                .translate(placement_dx, placement_dy);
            vec![RenderCommand::GraphicsContextViewport {
                object_id: command.object_id,
                viewport,
                zoom_raw: state.zoom_raw,
            }]
        }
        0x12 => {
            let Some(target_id) = decode_graphics_context_object_id(&command.payload) else {
                return Vec::new();
            };
            if target_id == command.object_id
                || object_contains(pool, target_id, command.object_id, &mut Vec::new())
            {
                return Vec::new();
            }
            let (x, y) = state.device_point(viewport);
            let object_scene = engine.build_object_at(pool, target_id, x, y);
            let target_rect = object_scene
                .find(target_id)
                .map(|node| node.rect)
                .or_else(|| scene_bounds(&object_scene));
            if let Some(target_rect) = target_rect {
                state.cursor_x = target_rect
                    .right()
                    .saturating_sub(1)
                    .saturating_sub(viewport.x);
                state.cursor_y = target_rect
                    .bottom()
                    .saturating_sub(1)
                    .saturating_sub(viewport.y);
            }
            let mut commands = renderer.render(&object_scene);
            let prefix = commands
                .iter()
                .take(2)
                .filter(|command| {
                    matches!(
                        command,
                        RenderCommand::FillRect { .. } | RenderCommand::Clip(_)
                    )
                })
                .count();
            commands.drain(0..prefix);
            state.canvas_render_commands(&commands, renderer.palette(), viewport);
            commands
        }
        0x13 | 0x14 => {
            let Some(picture_id) = decode_graphics_context_object_id(&command.payload) else {
                return Vec::new();
            };
            if !pool
                .find(picture_id)
                .is_some_and(|object| object.r#type == ObjectType::PictureGraphic)
            {
                return Vec::new();
            }
            let source = if command.subcommand == 0x13 {
                GraphicsContextCopySource::Canvas
            } else {
                GraphicsContextCopySource::Viewport
            };
            let mut commands = vec![RenderCommand::GraphicsContextCopyToPicture {
                object_id: command.object_id,
                picture_id,
                source,
                viewport: state
                    .effective_viewport(canvas_viewport_base)
                    .translate(placement_dx, placement_dy),
                zoom_raw: state.zoom_raw,
            }];
            if let Some((width, height, data)) =
                state.copy_pixels_for_picture(pool, picture_id, source, canvas_viewport_base)
            {
                commands.push(RenderCommand::GraphicsContextPictureData {
                    object_id: command.object_id,
                    picture_id,
                    source,
                    width,
                    height,
                    format: 2,
                    transparent_index: state.transparency_colour(),
                    data,
                });
            }
            commands
        }
        _ => Vec::new(),
    }
}

fn scene_bounds(scene: &Scene) -> Option<Rect> {
    let mut nodes = scene.visible_nodes();
    let first = nodes.next()?.rect;
    let (mut left, mut top, mut right, mut bottom) =
        (first.x, first.y, first.right(), first.bottom());
    for node in nodes {
        left = left.min(node.rect.x);
        top = top.min(node.rect.y);
        right = right.max(node.rect.right());
        bottom = bottom.max(node.rect.bottom());
    }
    let width = u16::try_from(right.saturating_sub(left)).unwrap_or(u16::MAX);
    let height = u16::try_from(bottom.saturating_sub(top)).unwrap_or(u16::MAX);
    Some(Rect::new(left, top, width, height))
}

fn object_contains(
    pool: &ObjectPool,
    root: ObjectID,
    needle: ObjectID,
    path: &mut Vec<ObjectID>,
) -> bool {
    if root == needle {
        return true;
    }
    if path.contains(&root) {
        return false;
    }
    path.push(root);
    let contains = pool.find(root).is_some_and(|object| {
        object
            .children_pos
            .iter()
            .any(|child| object_contains(pool, child.id, needle, path))
            || object
                .children
                .iter()
                .copied()
                .any(|child| object_contains(pool, child, needle, path))
    });
    path.pop();
    contains
}

#[derive(Debug, Clone)]
struct GraphicsContextDrawState {
    cursor_x: i32,
    cursor_y: i32,
    foreground_index: u8,
    foreground: Colour,
    text_foreground_index: u8,
    text_foreground: Colour,
    background_index: u8,
    background: Colour,
    fill_index: u8,
    fill_colour: Colour,
    fill_type: FillType,
    use_attribute_colours: bool,
    line_width: u16,
    line_art: u16,
    line_enabled: bool,
    font: FontMetrics,
    decoration: FontDecoration,
    viewport: Option<Rect>,
    zoom_raw: Option<u32>,
    canvas: Option<GraphicsContextCanvasState>,
}

impl GraphicsContextDrawState {
    fn new(
        pool: &ObjectPool,
        id: ObjectID,
        renderer: &GtuiRenderer,
        background_override: Option<u8>,
    ) -> Self {
        let body = pool
            .find(id)
            .and_then(|object| object.get_graphic_context_body().ok())
            .unwrap_or_default();
        let foreground_index = body.foreground_colour;
        let background_index = background_override.unwrap_or(body.background_colour);
        let mut state = Self {
            cursor_x: i32::from(body.cursor_x),
            cursor_y: i32::from(body.cursor_y),
            foreground_index,
            foreground: renderer.palette().resolve(foreground_index),
            text_foreground_index: foreground_index,
            text_foreground: renderer.palette().resolve(foreground_index),
            background_index,
            background: renderer.palette().resolve(background_index),
            fill_index: 0,
            fill_colour: renderer.palette().resolve(0),
            fill_type: FillType::None,
            use_attribute_colours: body.options & 0x02 != 0,
            line_width: 1,
            line_art: 0xFFFF,
            line_enabled: true,
            font: FontMetrics::default(),
            decoration: FontDecoration::default(),
            viewport: None,
            zoom_raw: Some(body.viewport_zoom_raw),
            canvas: GraphicsContextCanvasState::new(pool, id, background_index),
        };
        if body.line_attributes != ObjectID::NULL || state.use_attribute_colours {
            state.apply_line_attributes(pool, renderer, body.line_attributes);
        }
        if body.fill_attributes != ObjectID::NULL {
            state.apply_fill_attributes(pool, renderer, body.fill_attributes);
        }
        if body.font_attributes != ObjectID::NULL {
            state.apply_font_attributes(pool, renderer, body.font_attributes);
        }
        state
    }

    fn device_point(&self, viewport: Rect) -> (i32, i32) {
        (
            viewport.x.saturating_add(self.cursor_x),
            viewport.y.saturating_add(self.cursor_y),
        )
    }

    fn rect_at_cursor(&self, viewport: Rect, w: u16, h: u16) -> Rect {
        let (x, y) = self.device_point(viewport);
        Rect::new(x, y, w, h)
    }

    fn move_to_bottom_right_inside(&mut self, w: u16, h: u16) {
        self.cursor_x = self.cursor_x.saturating_add(i32::from(w.saturating_sub(1)));
        self.cursor_y = self.cursor_y.saturating_add(i32::from(h.saturating_sub(1)));
    }

    fn effective_viewport(&self, base: Rect) -> Rect {
        self.viewport.unwrap_or(base)
    }

    fn set_viewport_position(&mut self, base: Rect, x: i32, y: i32) {
        let current = self.effective_viewport(base);
        self.viewport = Some(Rect::new(x, y, current.w, current.h));
    }

    fn set_viewport_size(&mut self, base: Rect, w: u16, h: u16) {
        let current = self.effective_viewport(base);
        self.viewport = Some(Rect::new(current.x, current.y, w, h));
    }

    fn apply_line_attributes(&mut self, pool: &ObjectPool, renderer: &GtuiRenderer, id: ObjectID) {
        if id == ObjectID::NULL {
            self.line_enabled = false;
            return;
        }
        let Some(obj) = pool.find(id) else {
            return;
        };
        if obj.r#type != ObjectType::LineAttributes {
            return;
        }
        if let Ok(body) = obj.get_line_attributes_body() {
            if self.use_attribute_colours {
                self.foreground_index = body.line_color;
                self.foreground = renderer.palette().resolve(body.line_color);
            }
            self.line_width = u16::from(body.line_width);
            self.line_art = body.line_art;
            self.line_enabled = body.line_width != 0;
        }
    }

    fn apply_fill_attributes(&mut self, pool: &ObjectPool, renderer: &GtuiRenderer, id: ObjectID) {
        if id == ObjectID::NULL {
            self.fill_type = FillType::None;
            return;
        }
        let Some(obj) = pool.find(id) else {
            return;
        };
        if obj.r#type != ObjectType::FillAttributes {
            return;
        }
        if let Ok(body) = obj.get_fill_attributes_body() {
            self.fill_type = FillType::from_u8(body.fill_type);
            if self.use_attribute_colours {
                self.fill_index = body.fill_color;
                self.fill_colour = renderer.palette().resolve(body.fill_color);
            } else {
                self.fill_index = self.background_index;
                self.fill_colour = self.background;
            }
        }
    }

    fn apply_font_attributes(&mut self, pool: &ObjectPool, renderer: &GtuiRenderer, id: ObjectID) {
        if id == ObjectID::NULL {
            self.text_foreground_index = 1;
            self.text_foreground = renderer.palette().resolve(1);
            self.font = FontMetrics::default();
            self.decoration = FontDecoration::default();
            return;
        }
        let Some(obj) = pool.find(id) else {
            return;
        };
        if obj.r#type != ObjectType::FontAttributes {
            return;
        }
        if let Ok(body) = obj.get_font_attributes_body() {
            if self.use_attribute_colours {
                self.text_foreground_index = body.font_color;
                self.text_foreground = renderer.palette().resolve(body.font_color);
            } else {
                self.text_foreground_index = self.foreground_index;
                self.text_foreground = self.foreground;
            }
            self.font = FontMetrics::for_size(body.font_size);
            self.decoration = FontDecoration::from_style_byte(body.font_style);
        }
    }

    const fn is_shape_filled(&self) -> bool {
        !matches!(self.fill_type, FillType::None)
    }

    const fn effective_fill_colour(&self) -> Colour {
        match self.fill_type {
            FillType::LineColour => self.foreground,
            FillType::FillColour | FillType::Pattern => self.fill_colour,
            FillType::None => self.background,
        }
    }

    const fn effective_fill_index(&self) -> u8 {
        match self.fill_type {
            FillType::LineColour => self.foreground_index,
            FillType::FillColour | FillType::Pattern => self.fill_index,
            FillType::None => self.background_index,
        }
    }

    fn canvas_set_current_pixel(&mut self, colour: u8) {
        if let Some(canvas) = &mut self.canvas {
            canvas.set_pixel(self.cursor_x, self.cursor_y, colour);
        }
    }

    fn fill_canvas_rect_at_cursor(&mut self, w: u16, h: u16, colour: u8) {
        if let Some(canvas) = &mut self.canvas {
            canvas.fill_rect(self.cursor_x, self.cursor_y, w, h, colour);
        }
    }

    fn stroke_canvas_rect_at_cursor(
        &mut self,
        w: u16,
        h: u16,
        colour: u8,
        width: u16,
        line_art: u16,
    ) {
        if let Some(canvas) = &mut self.canvas {
            canvas.stroke_rect_suppressed(
                Rect::new(self.cursor_x, self.cursor_y, w, h),
                colour,
                width,
                line_art,
                0,
            );
        }
    }

    fn canvas_line(
        &mut self,
        from: (i32, i32),
        to: (i32, i32),
        colour: u8,
        width: u16,
        line_art: u16,
    ) {
        if let Some(canvas) = &mut self.canvas {
            canvas.line(from, to, colour, width, line_art);
        }
    }

    fn canvas_ellipse_at_cursor(&mut self, w: u16, h: u16, style: CanvasDrawStyle) {
        if let Some(canvas) = &mut self.canvas {
            canvas.ellipse(self.cursor_x, self.cursor_y, w, h, style);
        }
    }

    fn canvas_polygon(&mut self, points: &[(i32, i32)], style: CanvasDrawStyle) {
        if let Some(canvas) = &mut self.canvas {
            canvas.polygon(points, style);
        }
    }

    fn canvas_draw_text(
        &mut self,
        rect_w: u16,
        rect_h: u16,
        transparent: bool,
        layout: &text_layout::TextLayout,
    ) {
        let Some(canvas) = &mut self.canvas else {
            return;
        };
        let origin_x = self.cursor_x;
        let origin_y = self.cursor_y;
        if !transparent {
            canvas.fill_rect(origin_x, origin_y, rect_w, rect_h, self.background_index);
        }

        let cell_w = self.font.cell_w.max(1);
        let cell_h = self.font.cell_h.max(1);
        for line in &layout.lines {
            for (col, ch) in line.text.chars().enumerate() {
                if ch.is_whitespace() {
                    continue;
                }
                let x = origin_x
                    .saturating_add(line.x_offset)
                    .saturating_add(usize_to_i32(col.saturating_mul(usize::from(cell_w))));
                let y = origin_y.saturating_add(line.y_offset);
                canvas.fill_rect(x, y, cell_w, cell_h, self.text_foreground_index);
            }
        }
    }

    fn canvas_render_commands(
        &mut self,
        commands: &[RenderCommand],
        palette: &Palette,
        viewport: Rect,
    ) {
        let Some(canvas) = &mut self.canvas else {
            return;
        };
        for command in commands {
            canvas.apply_render_command(command, palette, viewport);
        }
    }

    fn copy_pixels_for_picture(
        &self,
        pool: &ObjectPool,
        picture_id: ObjectID,
        source: GraphicsContextCopySource,
        base_viewport: Rect,
    ) -> Option<(u16, u16, Vec<u8>)> {
        let picture = pool.find(picture_id)?.get_picture_graphic_body().ok()?;
        let width = if picture.actual_width == 0 {
            picture.width
        } else {
            picture.actual_width
        };
        let height = picture.actual_height;
        if width == 0 || height == 0 {
            return None;
        }
        let canvas = self.canvas.as_ref()?;
        let (src_x, src_y) = match source {
            GraphicsContextCopySource::Canvas => (0, 0),
            GraphicsContextCopySource::Viewport => {
                let viewport = self.effective_viewport(base_viewport);
                (viewport.x, viewport.y)
            }
        };
        let (width, height, mut data) = match source {
            GraphicsContextCopySource::Canvas => canvas.copy_rect(src_x, src_y, width, height),
            GraphicsContextCopySource::Viewport => {
                canvas.copy_rect_with_zoom(src_x, src_y, width, height, self.viewport_zoom())
            }
        }?;
        let transparent_index = canvas.transparency_colour;
        data.iter_mut()
            .filter(|index| !picture_graphic_format_contains_index(picture.format, **index))
            .for_each(|index| *index = transparent_index);
        Some((width, height, data))
    }

    fn viewport_zoom(&self) -> f32 {
        self.zoom_raw
            .map(f32::from_bits)
            .filter(|zoom| zoom.is_finite() && *zoom > 0.0)
            .unwrap_or(1.0)
    }

    fn transparency_colour(&self) -> u8 {
        self.canvas
            .as_ref()
            .map_or(0, |canvas| canvas.transparency_colour)
    }

    fn text_style(&self) -> ResolvedStyle {
        ResolvedStyle {
            foreground: self.text_foreground,
            background: self.background,
            font: self.font,
            decoration: self.decoration,
            line_width: self.line_width,
            line_art: self.line_art,
            fill_type: self.fill_type,
            fill_colour: self.effective_fill_colour(),
        }
    }
}

const fn picture_graphic_format_contains_index(format: u8, index: u8) -> bool {
    match format {
        0 => index <= 1,
        1 => index <= 0x0F,
        2 => true,
        _ => false,
    }
}
