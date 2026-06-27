const MAX_GRAPHICS_CONTEXT_CANVAS_PIXELS: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct CanvasDrawStyle {
    filled: bool,
    fill_colour: u8,
    line_colour: u8,
    line_enabled: bool,
    line_width: u16,
    line_art: u16,
}

#[derive(Debug, Clone)]
struct GraphicsContextCanvasState {
    width: u16,
    height: u16,
    format: u8,
    transparency_colour: u8,
    pixels: Vec<u8>,
}

impl GraphicsContextCanvasState {
    fn new(pool: &ObjectPool, id: ObjectID, background: u8) -> Option<Self> {
        let body = pool.find(id)?.get_graphic_context_body().ok()?;
        let width = body.canvas_width;
        let height = body.canvas_height;
        if width == 0 || height == 0 {
            return None;
        }
        let pixels = usize::from(width).checked_mul(usize::from(height))?;
        if pixels > MAX_GRAPHICS_CONTEXT_CANVAS_PIXELS {
            return None;
        }
        let initial = if body.options & 0x01 != 0 {
            body.transparency_colour
        } else if graphics_context_format_contains_index(body.format, background) {
            background
        } else {
            body.transparency_colour
        };
        Some(Self {
            width,
            height,
            format: body.format,
            transparency_colour: body.transparency_colour,
            pixels: vec![initial; pixels],
        })
    }

    fn set_pixel(&mut self, x: i32, y: i32, colour: u8) {
        let colour = self.normalise_colour(colour);
        if x < 0 || y < 0 {
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
        let index = usize::from(y)
            .saturating_mul(usize::from(self.width))
            .saturating_add(usize::from(x));
        if let Some(pixel) = self.pixels.get_mut(index) {
            *pixel = colour;
        }
    }

    fn normalise_colour(&self, colour: u8) -> u8 {
        if graphics_context_format_contains_index(self.format, colour) {
            colour
        } else {
            self.transparency_colour
        }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: u16, h: u16, colour: u8) {
        for yy in 0..h {
            for xx in 0..w {
                self.set_pixel(
                    x.saturating_add(i32::from(xx)),
                    y.saturating_add(i32::from(yy)),
                    colour,
                );
            }
        }
    }

    fn stroke_rect(&mut self, x: i32, y: i32, w: u16, h: u16, colour: u8, width: u16) {
        if w == 0 || h == 0 {
            return;
        }
        for inset in 0..width.min(w).min(h) {
            let x = x.saturating_add(i32::from(inset));
            let y = y.saturating_add(i32::from(inset));
            let w = w.saturating_sub(inset.saturating_mul(2));
            let h = h.saturating_sub(inset.saturating_mul(2));
            if w == 0 || h == 0 {
                continue;
            }
            let right = x.saturating_add(i32::from(w.saturating_sub(1)));
            let bottom = y.saturating_add(i32::from(h.saturating_sub(1)));
            for xx in 0..w {
                let px = x.saturating_add(i32::from(xx));
                self.set_pixel(px, y, colour);
                self.set_pixel(px, bottom, colour);
            }
            for yy in 0..h {
                let py = y.saturating_add(i32::from(yy));
                self.set_pixel(x, py, colour);
                self.set_pixel(right, py, colour);
            }
        }
    }

    fn stroke_rect_suppressed(
        &mut self,
        rect: Rect,
        colour: u8,
        width: u16,
        line_art: u16,
        suppress: u8,
    ) {
        let Rect { x, y, w, h } = rect;
        if w == 0 || h == 0 {
            return;
        }
        if line_art != 0xFFFF {
            let right = x.saturating_add(i32::from(w.saturating_sub(1)));
            let bottom = y.saturating_add(i32::from(h.saturating_sub(1)));
            if suppress & 0x01 == 0 {
                self.line((x, y), (right, y), colour, width, line_art);
            }
            if suppress & 0x02 == 0 {
                self.line((right, y), (right, bottom), colour, width, line_art);
            }
            if suppress & 0x04 == 0 {
                self.line((x, bottom), (right, bottom), colour, width, line_art);
            }
            if suppress & 0x08 == 0 {
                self.line((x, y), (x, bottom), colour, width, line_art);
            }
            return;
        }
        for inset in 0..width.min(w).min(h) {
            let x = x.saturating_add(i32::from(inset));
            let y = y.saturating_add(i32::from(inset));
            let w = w.saturating_sub(inset.saturating_mul(2));
            let h = h.saturating_sub(inset.saturating_mul(2));
            if w == 0 || h == 0 {
                continue;
            }
            let right = x.saturating_add(i32::from(w.saturating_sub(1)));
            let bottom = y.saturating_add(i32::from(h.saturating_sub(1)));
            if suppress & 0x01 == 0 {
                for xx in 0..w {
                    self.set_pixel(x.saturating_add(i32::from(xx)), y, colour);
                }
            }
            if suppress & 0x02 == 0 {
                for yy in 0..h {
                    self.set_pixel(right, y.saturating_add(i32::from(yy)), colour);
                }
            }
            if suppress & 0x04 == 0 {
                for xx in 0..w {
                    self.set_pixel(x.saturating_add(i32::from(xx)), bottom, colour);
                }
            }
            if suppress & 0x08 == 0 {
                for yy in 0..h {
                    self.set_pixel(x, y.saturating_add(i32::from(yy)), colour);
                }
            }
        }
    }

    fn line(&mut self, from: (i32, i32), to: (i32, i32), colour: u8, width: u16, line_art: u16) {
        if width == 0 {
            return;
        }
        let (mut x0, mut y0) = from;
        let (x1, y1) = to;
        let dx = x1.saturating_sub(x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -y1.saturating_sub(y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx.saturating_add(dy);
        let mut step = 0usize;
        loop {
            if line_art_bit(line_art, step) {
                self.thick_point(x0, y0, width, colour);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            step = step.saturating_add(1);
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

    fn thick_point(&mut self, x: i32, y: i32, width: u16, colour: u8) {
        if width == 0 {
            return;
        }
        let before = i32::from(width.saturating_sub(1) / 2);
        let after = i32::from(width / 2);
        for yy in -before..=after {
            for xx in -before..=after {
                self.set_pixel(x.saturating_add(xx), y.saturating_add(yy), colour);
            }
        }
    }

    fn ellipse(&mut self, x: i32, y: i32, w: u16, h: u16, style: CanvasDrawStyle) {
        if w == 0 || h == 0 {
            return;
        }
        let w_i = i64::from(w);
        let h_i = i64::from(h);
        let threshold = w_i * w_i * h_i * h_i;
        let cx2 = i64::from(x)
            .saturating_mul(2)
            .saturating_add(i64::from(w.saturating_sub(1)));
        let cy2 = i64::from(y)
            .saturating_mul(2)
            .saturating_add(i64::from(h.saturating_sub(1)));
        for yy in 0..h {
            for xx in 0..w {
                let px = x.saturating_add(i32::from(xx));
                let py = y.saturating_add(i32::from(yy));
                let dx2 = i64::from(px).saturating_mul(2).saturating_sub(cx2);
                let dy2 = i64::from(py).saturating_mul(2).saturating_sub(cy2);
                let value = dx2
                    .saturating_mul(dx2)
                    .saturating_mul(h_i)
                    .saturating_mul(h_i)
                    .saturating_add(
                        dy2.saturating_mul(dy2)
                            .saturating_mul(w_i)
                            .saturating_mul(w_i),
                    );
                if value <= threshold {
                    if style.filled {
                        self.set_pixel(px, py, style.fill_colour);
                    }
                    if style.line_enabled
                        && style.line_art == 0xFFFF
                        && (!Self::ellipse_contains(px - 1, py, cx2, cy2, w_i, h_i, threshold)
                            || !Self::ellipse_contains(
                                px.saturating_add(1),
                                py,
                                cx2,
                                cy2,
                                w_i,
                                h_i,
                                threshold,
                            )
                            || !Self::ellipse_contains(px, py - 1, cx2, cy2, w_i, h_i, threshold)
                            || !Self::ellipse_contains(
                                px,
                                py.saturating_add(1),
                                cx2,
                                cy2,
                                w_i,
                                h_i,
                                threshold,
                            ))
                    {
                        self.thick_point(px, py, style.line_width, style.line_colour);
                    }
                }
            }
        }
        if style.line_enabled && style.line_art != 0xFFFF {
            let points = Self::ellipse_arc_points(Rect::new(x, y, w, h), 0, 180);
            self.polyline(&points, style.line_colour, style.line_width, style.line_art);
        }
    }

    fn ellipse_arc(
        &mut self,
        rect: Rect,
        ellipse_type: u8,
        start_angle: u8,
        end_angle: u8,
        style: CanvasDrawStyle,
    ) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        let mut points = Self::ellipse_arc_points(rect, start_angle, end_angle);
        if points.len() < 2 {
            return;
        }

        match ellipse_type {
            2 => {
                if style.filled && points.len() >= 3 {
                    self.polygon(
                        &points,
                        CanvasDrawStyle {
                            filled: true,
                            line_enabled: false,
                            ..style
                        },
                    );
                }
                if style.line_enabled {
                    let first = points[0];
                    points.push(first);
                    self.polyline(&points, style.line_colour, style.line_width, style.line_art);
                }
            }
            3 => {
                let centre = Self::ellipse_centre_point(rect);
                let mut section = Vec::with_capacity(points.len() + 2);
                section.push(centre);
                section.extend(points);
                section.push(centre);
                if style.filled && section.len() >= 3 {
                    self.polygon(
                        &section,
                        CanvasDrawStyle {
                            filled: true,
                            line_enabled: false,
                            ..style
                        },
                    );
                }
                if style.line_enabled {
                    self.polyline(
                        &section,
                        style.line_colour,
                        style.line_width,
                        style.line_art,
                    );
                }
            }
            _ => {
                if style.line_enabled {
                    self.polyline(&points, style.line_colour, style.line_width, style.line_art);
                }
            }
        }
    }

    fn polyline(&mut self, points: &[(i32, i32)], colour: u8, width: u16, line_art: u16) {
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], colour, width, line_art);
        }
    }

    fn meter_ticks(
        &mut self,
        rect: Rect,
        start_angle: u8,
        end_angle: u8,
        number_of_ticks: u8,
        colour: u8,
    ) {
        if number_of_ticks == 0 {
            return;
        }
        let centre = Self::ellipse_centre_point(rect);
        let denominator = u32::from(number_of_ticks.saturating_sub(1)).max(1);
        for tick in 0..number_of_ticks {
            let fraction = f64::from(tick) / f64::from(denominator);
            let angle = Self::angle_for_fraction(start_angle, end_angle, false, fraction);
            let outer = Self::ellipse_border_point(rect, angle);
            let inner = (
                centre
                    .0
                    .saturating_add((outer.0.saturating_sub(centre.0)) * 3 / 4),
                centre
                    .1
                    .saturating_add((outer.1.saturating_sub(centre.1)) * 3 / 4),
            );
            self.line(inner, outer, colour, 1, 0xFFFF);
        }
    }

    fn linear_bar_ticks(
        &mut self,
        rect: Rect,
        horizontal: bool,
        direction_positive: bool,
        ticks: u8,
        colour: u8,
    ) {
        if ticks == 0 {
            return;
        }
        let denominator = u32::from(ticks.saturating_sub(1)).max(1);
        for tick in 0..ticks {
            let fraction = if ticks == 1 {
                0.5
            } else {
                f64::from(tick) / f64::from(denominator)
            };
            self.linear_bar_marker(rect, horizontal, direction_positive, fraction, colour);
        }
    }

    fn linear_bar_marker(
        &mut self,
        rect: Rect,
        horizontal: bool,
        direction_positive: bool,
        fraction: f64,
        colour: u8,
    ) {
        let fraction = fraction.clamp(0.0, 1.0);
        if horizontal {
            let span = f64::from(rect.w.saturating_sub(1));
            let offset = (span * fraction).round() as i32;
            let x = if direction_positive {
                rect.x.saturating_add(offset)
            } else {
                rect.x
                    .saturating_add(i32::from(rect.w.saturating_sub(1)))
                    .saturating_sub(offset)
            };
            self.line(
                (x, rect.y),
                (
                    x,
                    rect.y.saturating_add(i32::from(rect.h.saturating_sub(1))),
                ),
                colour,
                1,
                0xFFFF,
            );
        } else {
            let span = f64::from(rect.h.saturating_sub(1));
            let offset = (span * fraction).round() as i32;
            let y = if direction_positive {
                rect.y
                    .saturating_add(i32::from(rect.h.saturating_sub(1)))
                    .saturating_sub(offset)
            } else {
                rect.y.saturating_add(offset)
            };
            self.line(
                (rect.x, y),
                (
                    rect.x.saturating_add(i32::from(rect.w.saturating_sub(1))),
                    y,
                ),
                colour,
                1,
                0xFFFF,
            );
        }
    }

    fn radial_bar_marker(&mut self, rect: Rect, angle: f64, colour: u8, width: u16) {
        let centre = Self::ellipse_centre_point(rect);
        let outer = Self::ellipse_border_point(rect, angle);
        let inner = (
            centre
                .0
                .saturating_add((outer.0.saturating_sub(centre.0)) / 2),
            centre
                .1
                .saturating_add((outer.1.saturating_sub(centre.1)) / 2),
        );
        self.line(inner, outer, colour, width, 0xFFFF);
    }

    fn ellipse_arc_points(rect: Rect, start_angle: u8, end_angle: u8) -> Vec<(i32, i32)> {
        let start = f64::from(start_angle) * 2.0;
        let mut end = f64::from(end_angle) * 2.0;
        while end <= start {
            end += 360.0;
        }
        let sweep = end - start;
        let steps = ((sweep / 5.0).ceil() as usize).clamp(2, 96);
        let mut points = Vec::with_capacity(steps + 1);
        for step in 0..=steps {
            let angle = start + sweep * (step as f64 / steps as f64);
            let point = Self::ellipse_border_point(rect, angle);
            if points.last().copied() != Some(point) {
                points.push(point);
            }
        }
        points
    }

    fn value_fraction(value: u32, min: i32, max: i32) -> f64 {
        let min = i64::from(min);
        let max = i64::from(max);
        let range = max.saturating_sub(min).max(1);
        let position = i64::from(value).saturating_sub(min).clamp(0, range);
        position as f64 / range as f64
    }

    fn scaled_extent(extent: u16, fraction: f64) -> u16 {
        let scaled = f64::from(extent) * fraction.clamp(0.0, 1.0);
        u16::try_from(scaled.round() as u32)
            .unwrap_or(extent)
            .min(extent)
    }

    fn angle_for_fraction(start_angle: u8, end_angle: u8, clockwise: bool, fraction: f64) -> f64 {
        let start = f64::from(start_angle) * 2.0;
        let mut end = f64::from(end_angle) * 2.0;
        if clockwise {
            while end >= start {
                end -= 360.0;
            }
        } else {
            while end <= start {
                end += 360.0;
            }
        }
        start + (end - start) * fraction.clamp(0.0, 1.0)
    }

    fn arc_points_for_fraction(
        rect: Rect,
        start_angle: u8,
        end_angle: u8,
        clockwise: bool,
        fraction: f64,
    ) -> Vec<(i32, i32)> {
        let start = f64::from(start_angle) * 2.0;
        let end = Self::angle_for_fraction(start_angle, end_angle, clockwise, fraction);
        let sweep = end - start;
        let steps = ((sweep.abs() / 5.0).ceil() as usize).clamp(2, 96);
        let mut points = Vec::with_capacity(steps + 1);
        for step in 0..=steps {
            let angle = start + sweep * (step as f64 / steps as f64);
            let point = Self::ellipse_border_point(rect, angle);
            if points.last().copied() != Some(point) {
                points.push(point);
            }
        }
        points
    }

    fn ellipse_centre_point(rect: Rect) -> (i32, i32) {
        (
            rect.x + i32::from(rect.w.saturating_sub(1)) / 2,
            rect.y + i32::from(rect.h.saturating_sub(1)) / 2,
        )
    }

    fn ellipse_border_point(rect: Rect, degrees: f64) -> (i32, i32) {
        let cx = f64::from(rect.x) + f64::from(rect.w.saturating_sub(1)) / 2.0;
        let cy = f64::from(rect.y) + f64::from(rect.h.saturating_sub(1)) / 2.0;
        let rx = (f64::from(rect.w.saturating_sub(1)) / 2.0).max(0.5);
        let ry = (f64::from(rect.h.saturating_sub(1)) / 2.0).max(0.5);
        let radians = degrees.to_radians();
        let dx = radians.cos();
        let dy = radians.sin();
        let denominator = (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry);
        let scale = if denominator > 0.0 {
            1.0 / denominator.sqrt()
        } else {
            0.0
        };
        (
            (cx + scale * dx).round() as i32,
            (cy - scale * dy).round() as i32,
        )
    }

    fn ellipse_contains(
        px: i32,
        py: i32,
        cx2: i64,
        cy2: i64,
        w: i64,
        h: i64,
        threshold: i64,
    ) -> bool {
        let dx2 = i64::from(px).saturating_mul(2).saturating_sub(cx2);
        let dy2 = i64::from(py).saturating_mul(2).saturating_sub(cy2);
        dx2.saturating_mul(dx2)
            .saturating_mul(h)
            .saturating_mul(h)
            .saturating_add(dy2.saturating_mul(dy2).saturating_mul(w).saturating_mul(w))
            <= threshold
    }

    fn polygon(&mut self, points: &[(i32, i32)], style: CanvasDrawStyle) {
        if points.len() < 2 {
            return;
        }
        if style.filled && points.len() >= 3 {
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
                    if Self::point_in_polygon(x, y, points) {
                        self.set_pixel(x, y, style.fill_colour);
                    }
                }
            }
        }
        if style.line_enabled {
            for pair in points.windows(2) {
                self.line(
                    pair[0],
                    pair[1],
                    style.line_colour,
                    style.line_width,
                    style.line_art,
                );
            }
        }
    }

    fn pattern_rect(&mut self, rect: Rect, anchor: (i32, i32), pattern: &FillPattern) {
        self.pattern_pixels(rect, anchor, pattern, |_, _| true);
    }

    fn pattern_ellipse(
        &mut self,
        rect: Rect,
        anchor: (i32, i32),
        ellipse_type: u8,
        start_angle: u8,
        end_angle: u8,
        pattern: &FillPattern,
    ) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        if ellipse_type == 0 {
            let w_i = i64::from(rect.w);
            let h_i = i64::from(rect.h);
            let threshold = w_i * w_i * h_i * h_i;
            let cx2 = i64::from(rect.x)
                .saturating_mul(2)
                .saturating_add(i64::from(rect.w.saturating_sub(1)));
            let cy2 = i64::from(rect.y)
                .saturating_mul(2)
                .saturating_add(i64::from(rect.h.saturating_sub(1)));
            self.pattern_pixels(rect, anchor, pattern, |x, y| {
                Self::ellipse_contains(x, y, cx2, cy2, w_i, h_i, threshold)
            });
            return;
        }
        let mut points = Self::ellipse_arc_points(rect, start_angle, end_angle);
        match ellipse_type {
            2 if points.len() >= 3 => {
                let first = points[0];
                points.push(first);
                self.pattern_polygon(&points, anchor, pattern);
            }
            3 if points.len() >= 2 => {
                let centre = Self::ellipse_centre_point(rect);
                let mut section = Vec::with_capacity(points.len() + 2);
                section.push(centre);
                section.extend(points);
                section.push(centre);
                self.pattern_polygon(&section, anchor, pattern);
            }
            _ => {}
        }
    }

    fn pattern_polygon(&mut self, points: &[(i32, i32)], anchor: (i32, i32), pattern: &FillPattern) {
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
        let width = u16::try_from(right.saturating_sub(left).saturating_add(1)).unwrap_or(u16::MAX);
        let height = u16::try_from(bottom.saturating_sub(top).saturating_add(1)).unwrap_or(u16::MAX);
        self.pattern_pixels(Rect::new(left, top, width, height), anchor, pattern, |x, y| {
            Self::point_in_polygon(x, y, points)
        });
    }

    fn pattern_pixels(
        &mut self,
        rect: Rect,
        anchor: (i32, i32),
        pattern: &FillPattern,
        contains: impl Fn(i32, i32) -> bool,
    ) {
        if rect.w == 0 || rect.h == 0 || pattern.width == 0 || pattern.height == 0 {
            return;
        }
        let Some(data) = Self::pattern_data(pattern) else {
            return;
        };
        if !Self::indexed_bitmap_has_minimum_len(
            &data,
            pattern.format,
            pattern.width,
            pattern.height,
        ) {
            return;
        }
        for yy in 0..rect.h {
            for xx in 0..rect.w {
                let x = rect.x.saturating_add(i32::from(xx));
                let y = rect.y.saturating_add(i32::from(yy));
                if !contains(x, y) {
                    continue;
                }
                let pattern_x = Self::pattern_axis_index(x, anchor.0, pattern.width);
                let pattern_y = Self::pattern_axis_index(y, anchor.1, pattern.height);
                let Some(index) =
                    indexed_bitmap_pixel(&data, pattern.format, pattern.width, pattern_x, pattern_y)
                else {
                    return;
                };
                self.set_pixel(x, y, index);
            }
        }
    }

    fn pattern_axis_index(value: i32, anchor: i32, period: u16) -> usize {
        let period = i64::from(period);
        usize::try_from((i64::from(value) - i64::from(anchor)).rem_euclid(period)).unwrap_or(0)
    }

    fn pattern_data(pattern: &FillPattern) -> Option<Vec<u8>> {
        if !pattern.compressed {
            return Some(pattern.data.clone());
        }
        Self::decode_rle_pairs(&pattern.data)
    }

    fn indexed_bitmap_has_minimum_len(
        data: &[u8],
        format: u8,
        width: u16,
        height: u16,
    ) -> bool {
        let width = usize::from(width);
        let height = usize::from(height);
        let row = match format {
            0 => width.saturating_add(7) / 8,
            1 => width.saturating_add(1) / 2,
            2 => width,
            _ => return false,
        };
        row.checked_mul(height)
            .is_some_and(|required| data.len() >= required)
    }

    fn decode_rle_pairs(data: &[u8]) -> Option<Vec<u8>> {
        if !data.len().is_multiple_of(2) {
            return None;
        }
        let mut out = Vec::new();
        for pair in data.chunks_exact(2) {
            let next_len = out.len().checked_add(usize::from(pair[0]))?;
            if next_len > MAX_GRAPHICS_CONTEXT_CANVAS_PIXELS {
                return None;
            }
            out.resize(next_len, pair[1]);
        }
        Some(out)
    }

    fn point_in_polygon(x: i32, y: i32, points: &[(i32, i32)]) -> bool {
        let mut inside = false;
        let mut previous = points[points.len() - 1];
        for &current in points {
            let crosses = (current.1 > y) != (previous.1 > y);
            if crosses {
                let denominator = i64::from(previous.1.saturating_sub(current.1));
                let lhs = i64::from(x.saturating_sub(current.0)).saturating_mul(denominator);
                let rhs = i64::from(previous.0.saturating_sub(current.0))
                    .saturating_mul(i64::from(y.saturating_sub(current.1)));
                let left_of_intersection = if denominator > 0 {
                    lhs < rhs
                } else {
                    lhs > rhs
                };
                if left_of_intersection {
                    inside = !inside;
                }
            }
            previous = current;
        }
        inside
    }

    fn copy_rect(&self, x: i32, y: i32, w: u16, h: u16) -> Option<(u16, u16, Vec<u8>)> {
        let len = usize::from(w).checked_mul(usize::from(h))?;
        if len > MAX_GRAPHICS_CONTEXT_CANVAS_PIXELS {
            return None;
        }
        let mut out = Vec::with_capacity(len);
        for yy in 0..h {
            for xx in 0..w {
                let sx = x.saturating_add(i32::from(xx));
                let sy = y.saturating_add(i32::from(yy));
                let value = if sx < 0 || sy < 0 {
                    self.transparency_colour
                } else {
                    let sx = u16::try_from(sx).ok();
                    let sy = u16::try_from(sy).ok();
                    match (sx, sy) {
                        (Some(sx), Some(sy)) if sx < self.width && sy < self.height => {
                            let index = usize::from(sy)
                                .saturating_mul(usize::from(self.width))
                                .saturating_add(usize::from(sx));
                            self.pixels
                                .get(index)
                                .copied()
                                .unwrap_or(self.transparency_colour)
                        }
                        _ => self.transparency_colour,
                    }
                };
                out.push(value);
            }
        }
        Some((w, h, out))
    }

    fn copy_rect_with_zoom(
        &self,
        x: i32,
        y: i32,
        w: u16,
        h: u16,
        zoom: f32,
    ) -> Option<(u16, u16, Vec<u8>)> {
        let len = usize::from(w).checked_mul(usize::from(h))?;
        if len > MAX_GRAPHICS_CONTEXT_CANVAS_PIXELS {
            return None;
        }
        let zoom = if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            1.0
        };
        let mut out = Vec::with_capacity(len);
        for yy in 0..h {
            for xx in 0..w {
                let source_x = (f32::from(xx) / zoom).floor() as i32;
                let source_y = (f32::from(yy) / zoom).floor() as i32;
                out.push(
                    self.pixel_or_transparency(
                        x.saturating_add(source_x),
                        y.saturating_add(source_y),
                    ),
                );
            }
        }
        Some((w, h, out))
    }

    fn pixel_or_transparency(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 {
            return self.transparency_colour;
        }
        let sx = u16::try_from(x).ok();
        let sy = u16::try_from(y).ok();
        match (sx, sy) {
            (Some(sx), Some(sy)) if sx < self.width && sy < self.height => {
                let index = usize::from(sy)
                    .saturating_mul(usize::from(self.width))
                    .saturating_add(usize::from(sx));
                self.pixels
                    .get(index)
                    .copied()
                    .unwrap_or(self.transparency_colour)
            }
            _ => self.transparency_colour,
        }
    }

    fn apply_render_command(&mut self, command: &RenderCommand, palette: &Palette, viewport: Rect) {
        match command {
            RenderCommand::FillRect { rect, colour } => {
                self.fill_rect(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                    palette_index(palette, *colour),
                );
            }
            RenderCommand::StrokeRect {
                rect,
                colour,
                width,
                line_art,
                suppress,
            } => {
                self.stroke_rect_suppressed(
                    Rect::new(
                        rect.x.saturating_sub(viewport.x),
                        rect.y.saturating_sub(viewport.y),
                        rect.w,
                        rect.h,
                    ),
                    palette_index(palette, *colour),
                    *width,
                    *line_art,
                    *suppress,
                );
            }
            RenderCommand::Line {
                x0,
                y0,
                x1,
                y1,
                colour,
                width,
                line_art,
            } => self.line(
                (x0.saturating_sub(viewport.x), y0.saturating_sub(viewport.y)),
                (x1.saturating_sub(viewport.x), y1.saturating_sub(viewport.y)),
                palette_index(palette, *colour),
                *width,
                *line_art,
            ),
            RenderCommand::Ellipse {
                rect,
                colour,
                fill_colour,
                filled,
                width,
                line_art,
            } => self.ellipse(
                rect.x.saturating_sub(viewport.x),
                rect.y.saturating_sub(viewport.y),
                rect.w,
                rect.h,
                CanvasDrawStyle {
                    filled: *filled,
                    fill_colour: palette_index(palette, *fill_colour),
                    line_colour: palette_index(palette, *colour),
                    line_enabled: *width != 0,
                    line_width: *width,
                    line_art: *line_art,
                },
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
            } => self.ellipse_arc(
                Rect::new(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                ),
                *ellipse_type,
                *start_angle,
                *end_angle,
                CanvasDrawStyle {
                    filled: *filled,
                    fill_colour: palette_index(palette, *fill_colour),
                    line_colour: palette_index(palette, *colour),
                    line_enabled: *width != 0,
                    line_width: *width,
                    line_art: *line_art,
                },
            ),
            RenderCommand::Polygon {
                points,
                colour,
                fill_colour,
                filled,
                width,
                line_art,
                ..
            } => {
                let points = points
                    .iter()
                    .map(|(x, y)| (x.saturating_sub(viewport.x), y.saturating_sub(viewport.y)))
                    .collect::<Vec<_>>();
                self.polygon(
                    &points,
                    CanvasDrawStyle {
                        filled: *filled,
                        fill_colour: palette_index(palette, *fill_colour),
                        line_colour: palette_index(palette, *colour),
                        line_enabled: *width != 0,
                        line_width: *width,
                        line_art: *line_art,
                    },
                );
            }
            RenderCommand::PatternFillRect {
                rect,
                anchor,
                pattern,
            } => {
                self.pattern_rect(
                    Rect::new(
                        rect.x.saturating_sub(viewport.x),
                        rect.y.saturating_sub(viewport.y),
                        rect.w,
                        rect.h,
                    ),
                    (
                        anchor.0.saturating_sub(viewport.x),
                        anchor.1.saturating_sub(viewport.y),
                    ),
                    pattern,
                );
            }
            RenderCommand::PatternFillEllipse {
                rect,
                anchor,
                ellipse_type,
                start_angle,
                end_angle,
                pattern,
            } => self.pattern_ellipse(
                Rect::new(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                ),
                (
                    anchor.0.saturating_sub(viewport.x),
                    anchor.1.saturating_sub(viewport.y),
                ),
                *ellipse_type,
                *start_angle,
                *end_angle,
                pattern,
            ),
            RenderCommand::PatternFillPolygon {
                points,
                anchor,
                pattern,
                ..
            } => {
                let points = points
                    .iter()
                    .map(|(x, y)| (x.saturating_sub(viewport.x), y.saturating_sub(viewport.y)))
                    .collect::<Vec<_>>();
                self.pattern_polygon(
                    &points,
                    (
                        anchor.0.saturating_sub(viewport.x),
                        anchor.1.saturating_sub(viewport.y),
                    ),
                    pattern,
                );
            }
            RenderCommand::IndexedImage {
                rect,
                width,
                height,
                format,
                transparent,
                transparency,
                data,
                ..
            } => self.indexed_image(
                Rect::new(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                ),
                (*width, *height),
                *format,
                *transparent,
                *transparency,
                data,
            ),
            RenderCommand::RgbaImage {
                rect,
                width,
                height,
                data,
                ..
            } => self.rgba_image(
                Rect::new(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                ),
                (*width, *height),
                data,
                palette,
            ),
            RenderCommand::DrawText {
                rect,
                style,
                layout,
                ..
            } => self.draw_text_cells(
                rect.x.saturating_sub(viewport.x),
                rect.y.saturating_sub(viewport.y),
                palette_index(palette, style.foreground),
                style.font,
                layout,
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
            } => {
                let fraction = Self::value_fraction(*value, *min, *max);
                let local = Rect::new(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                );
                let colour = palette_index(palette, *colour);
                let target_line_colour = palette_index(palette, *target_line_colour);
                if *arched {
                    let width = (*bar_width).max(1);
                    if *show_border {
                        let border = Self::arc_points_for_fraction(
                            local,
                            *start_angle,
                            *end_angle,
                            *clockwise,
                            1.0,
                        );
                        self.polyline(&border, colour, width, 0xFFFF);
                    }
                    if *line_only {
                        self.radial_bar_marker(
                            local,
                            Self::angle_for_fraction(
                                *start_angle,
                                *end_angle,
                                *clockwise,
                                fraction,
                            ),
                            colour,
                            width,
                        );
                    } else {
                        let points = Self::arc_points_for_fraction(
                            local,
                            *start_angle,
                            *end_angle,
                            *clockwise,
                            fraction,
                        );
                        self.polyline(&points, colour, width, 0xFFFF);
                    }
                    if *show_target_line {
                        self.radial_bar_marker(
                            local,
                            Self::angle_for_fraction(
                                *start_angle,
                                *end_angle,
                                *clockwise,
                                Self::value_fraction(*target_value, *min, *max),
                            ),
                            target_line_colour,
                            width,
                        );
                    }
                } else if *horizontal {
                    if *line_only {
                        self.linear_bar_marker(local, true, *direction_positive, fraction, colour);
                    } else {
                        let filled_w = Self::scaled_extent(local.w, fraction);
                        let x = if *direction_positive {
                            local.x
                        } else {
                            local
                                .x
                                .saturating_add(i32::from(local.w.saturating_sub(filled_w)))
                        };
                        self.fill_rect(x, local.y, filled_w, local.h, colour);
                    }
                } else {
                    if *line_only {
                        self.linear_bar_marker(local, false, *direction_positive, fraction, colour);
                    } else {
                        let filled_h = Self::scaled_extent(local.h, fraction);
                        let y = if *direction_positive {
                            local
                                .y
                                .saturating_add(i32::from(local.h.saturating_sub(filled_h)))
                        } else {
                            local.y
                        };
                        self.fill_rect(local.x, y, local.w, filled_h, colour);
                    }
                }
                if !*arched {
                    if *show_ticks {
                        self.linear_bar_ticks(
                            local,
                            *horizontal,
                            *direction_positive,
                            *number_of_ticks,
                            colour,
                        );
                    }
                    if *show_target_line {
                        self.linear_bar_marker(
                            local,
                            *horizontal,
                            *direction_positive,
                            Self::value_fraction(*target_value, *min, *max),
                            target_line_colour,
                        );
                    }
                    if *show_border {
                        self.stroke_rect(local.x, local.y, local.w, local.h, colour, 1);
                    }
                }
            }
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
            } => {
                let local = Rect::new(
                    rect.x.saturating_sub(viewport.x),
                    rect.y.saturating_sub(viewport.y),
                    rect.w,
                    rect.h,
                );
                self.ellipse(
                    local.x,
                    local.y,
                    local.w,
                    local.h,
                    CanvasDrawStyle {
                        filled: false,
                        fill_colour: palette_index(palette, *border_colour),
                        line_colour: palette_index(palette, *border_colour),
                        line_enabled: true,
                        line_width: 1,
                        line_art: 0xFFFF,
                    },
                );
                let arc_points =
                    Self::arc_points_for_fraction(local, *start_angle, *end_angle, false, 1.0);
                self.polyline(&arc_points, palette_index(palette, *arc_colour), 1, 0xFFFF);
                self.meter_ticks(
                    local,
                    *start_angle,
                    *end_angle,
                    *number_of_ticks,
                    palette_index(palette, *arc_colour),
                );
                let fraction = Self::value_fraction(*value, *min, *max);
                let angle = Self::angle_for_fraction(*start_angle, *end_angle, false, fraction);
                let centre = Self::ellipse_centre_point(local);
                let tip = Self::ellipse_border_point(local, angle);
                self.line(
                    centre,
                    tip,
                    palette_index(palette, *needle_colour),
                    1,
                    0xFFFF,
                );
                if *show_value {
                    let text_rect = Rect::new(
                        local.x.saturating_add(i32::from(local.w / 4)),
                        local.y.saturating_add(i32::from(local.h / 2)),
                        local.w.saturating_sub(local.w / 2),
                        local.h.saturating_sub(local.h / 2),
                    );
                    let font = FontMetrics::default();
                    let layout = text_layout::layout_text(
                        &value.to_string(),
                        font,
                        text_rect.w,
                        text_rect.h,
                        HorizontalAlign::Left,
                        VerticalAlign::Top,
                        false,
                    );
                    self.draw_text_cells(
                        text_rect.x,
                        text_rect.y,
                        palette_index(palette, *needle_colour),
                        font,
                        &layout,
                    );
                }
            }
            RenderCommand::Clip(_)
            | RenderCommand::Placeholder { .. }
            | RenderCommand::SoftKey { .. }
            | RenderCommand::GraphicsContextViewport { .. }
            | RenderCommand::GraphicsContextCopyToPicture { .. }
            | RenderCommand::GraphicsContextPictureData { .. }
            | RenderCommand::GraphicsContextCanvas { .. }
            | RenderCommand::GraphicsContextReplay { .. } => {}
        }
    }

    fn indexed_image(
        &mut self,
        rect: Rect,
        source_size: (u16, u16),
        format: u8,
        transparent: bool,
        transparency: u8,
        data: &[u8],
    ) {
        let (source_width, source_height) = source_size;
        if rect.w == 0 || rect.h == 0 || source_width == 0 || source_height == 0 {
            return;
        }
        let pixel_count = usize::from(source_width).saturating_mul(usize::from(source_height));
        let required_bytes = match format {
            0 => pixel_count.saturating_add(7) / 8,
            1 => pixel_count.saturating_add(1) / 2,
            2 => pixel_count,
            _ => return,
        };
        if data.len() < required_bytes {
            return;
        }

        for yy in 0..rect.h {
            for xx in 0..rect.w {
                let source_x =
                    u32::from(xx).saturating_mul(u32::from(source_width)) / u32::from(rect.w);
                let source_y =
                    u32::from(yy).saturating_mul(u32::from(source_height)) / u32::from(rect.h);
                let source_x = usize::try_from(source_x).unwrap_or(usize::MAX);
                let source_y = usize::try_from(source_y).unwrap_or(usize::MAX);
                let Some(colour) =
                    indexed_bitmap_pixel(data, format, source_width, source_x, source_y)
                else {
                    return;
                };
                if !transparent || colour != transparency {
                    self.set_pixel(
                        rect.x.saturating_add(i32::from(xx)),
                        rect.y.saturating_add(i32::from(yy)),
                        colour,
                    );
                }
            }
        }
    }

    fn rgba_image(&mut self, rect: Rect, source_size: (u16, u16), data: &[u8], palette: &Palette) {
        let (source_width, source_height) = source_size;
        if rect.w == 0 || rect.h == 0 || source_width == 0 || source_height == 0 {
            return;
        }
        let required = usize::from(source_width)
            .saturating_mul(usize::from(source_height))
            .saturating_mul(4);
        if data.len() < required {
            return;
        }

        for yy in 0..rect.h {
            for xx in 0..rect.w {
                let source_x =
                    u32::from(xx).saturating_mul(u32::from(source_width)) / u32::from(rect.w);
                let source_y =
                    u32::from(yy).saturating_mul(u32::from(source_height)) / u32::from(rect.h);
                let pixel_index = usize::try_from(
                    source_y
                        .saturating_mul(u32::from(source_width))
                        .saturating_add(source_x),
                )
                .unwrap_or(usize::MAX);
                let Some(pixel) = pixel_index
                    .checked_mul(4)
                    .and_then(|start| data.get(start..start + 4))
                else {
                    return;
                };
                if pixel[3] != 0 {
                    self.set_pixel(
                        rect.x.saturating_add(i32::from(xx)),
                        rect.y.saturating_add(i32::from(yy)),
                        palette_index(palette, Colour::rgb(pixel[0], pixel[1], pixel[2])),
                    );
                }
            }
        }
    }

    fn draw_text_cells(
        &mut self,
        x: i32,
        y: i32,
        colour: u8,
        font: FontMetrics,
        layout: &text_layout::TextLayout,
    ) {
        let cell_w = font.cell_w.max(1);
        let cell_h = font.cell_h.max(1);
        for line in &layout.lines {
            for (col, ch) in line.text.chars().enumerate() {
                if ch.is_whitespace() {
                    continue;
                }
                self.fill_rect(
                    x.saturating_add(line.x_offset)
                        .saturating_add(usize_to_i32(col.saturating_mul(usize::from(cell_w)))),
                    y.saturating_add(line.y_offset),
                    cell_w,
                    cell_h,
                    colour,
                );
            }
        }
    }
}

const fn graphics_context_format_contains_index(format: u8, index: u8) -> bool {
    match format {
        0 => index <= 1,
        1 => index <= 0x0F,
        2 => true,
        _ => false,
    }
}
