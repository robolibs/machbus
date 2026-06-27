fn stroke_rect(
    fb: &mut Framebuffer,
    rect: Rect,
    colour: Colour,
    width: u16,
    line_art: u16,
    suppress: u8,
    clip: Rect,
) {
    if rect.w == 0 || rect.h == 0 {
        return;
    }
    if line_art != 0xFFFF {
        let right = rect.x.saturating_add(i32::from(rect.w.saturating_sub(1)));
        let bottom = rect.y.saturating_add(i32::from(rect.h.saturating_sub(1)));
        if suppress & 0x01 == 0 {
            draw_line(
                fb,
                LineStroke {
                    x0: rect.x,
                    y0: rect.y,
                    x1: right,
                    y1: rect.y,
                    colour,
                    width,
                    line_art,
                },
                clip,
            );
        }
        if suppress & 0x02 == 0 {
            draw_line(
                fb,
                LineStroke {
                    x0: right,
                    y0: rect.y,
                    x1: right,
                    y1: bottom,
                    colour,
                    width,
                    line_art,
                },
                clip,
            );
        }
        if suppress & 0x04 == 0 {
            draw_line(
                fb,
                LineStroke {
                    x0: rect.x,
                    y0: bottom,
                    x1: right,
                    y1: bottom,
                    colour,
                    width,
                    line_art,
                },
                clip,
            );
        }
        if suppress & 0x08 == 0 {
            draw_line(
                fb,
                LineStroke {
                    x0: rect.x,
                    y0: rect.y,
                    x1: rect.x,
                    y1: bottom,
                    colour,
                    width,
                    line_art,
                },
                clip,
            );
        }
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
        if suppress & 0x01 == 0 {
            for xx in 0..w {
                fb.set_pixel(x.saturating_add(i32::from(xx)), y, colour, clip);
            }
        }
        if suppress & 0x02 == 0 {
            for yy in 0..h {
                fb.set_pixel(right, y.saturating_add(i32::from(yy)), colour, clip);
            }
        }
        if suppress & 0x04 == 0 {
            for xx in 0..w {
                fb.set_pixel(x.saturating_add(i32::from(xx)), bottom, colour, clip);
            }
        }
        if suppress & 0x08 == 0 {
            for yy in 0..h {
                fb.set_pixel(x, y.saturating_add(i32::from(yy)), colour, clip);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LineStroke {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    colour: Colour,
    width: u16,
    line_art: u16,
}

fn draw_line(fb: &mut Framebuffer, line: LineStroke, clip: Rect) {
    let mut x0 = line.x0;
    let mut y0 = line.y0;
    let dx = line.x1.saturating_sub(x0).abs();
    let sx = if x0 < line.x1 { 1 } else { -1 };
    let dy = -line.y1.saturating_sub(y0).abs();
    let sy = if y0 < line.y1 { 1 } else { -1 };
    let mut err = dx.saturating_add(dy);
    let mut step = 0usize;
    loop {
        if line_art_bit(line.line_art, step) {
            draw_thick_point(fb, x0, y0, line.width, line.colour, clip);
        }
        if x0 == line.x1 && y0 == line.y1 {
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

#[inline]
fn line_art_bit(line_art: u16, step: usize) -> bool {
    let bit = 15usize.saturating_sub(step % 16);
    (line_art & (1u16 << bit)) != 0
}

#[derive(Debug, Clone, Copy)]
struct MeterDraw {
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
}

fn draw_meter(fb: &mut Framebuffer, meter: MeterDraw, clip: Rect) {
    if meter.rect.w == 0 || meter.rect.h == 0 {
        return;
    }
    draw_ellipse(
        fb,
        meter.rect,
        ShapeDrawStyle {
            line_colour: meter.border_colour,
            fill_colour: meter.border_colour,
            filled: false,
            line_enabled: true,
            line_width: 1,
            line_art: 0xFFFF,
        },
        clip,
    );
    let arc_points =
        arc_points_for_fraction(meter.rect, meter.start_angle, meter.end_angle, false, 1.0);
    draw_polyline(fb, &arc_points, meter.arc_colour, 1, 0xFFFF, clip);
    draw_meter_ticks(
        fb,
        meter.rect,
        meter.start_angle,
        meter.end_angle,
        meter.number_of_ticks,
        meter.arc_colour,
        clip,
    );

    let fraction = value_fraction(meter.value, meter.min, meter.max);
    let needle_angle = angle_for_fraction(meter.start_angle, meter.end_angle, false, fraction);
    let centre = ellipse_centre_point(meter.rect);
    let tip = ellipse_border_point(meter.rect, needle_angle);
    draw_line(
        fb,
        LineStroke {
            x0: centre.0,
            y0: centre.1,
            x1: tip.0,
            y1: tip.1,
            colour: meter.needle_colour,
            width: 1,
            line_art: 0xFFFF,
        },
        clip,
    );
    if meter.show_value {
        draw_label_cells(
            fb,
            Rect::new(
                meter.rect.x.saturating_add(i32::from(meter.rect.w / 4)),
                meter.rect.y.saturating_add(i32::from(meter.rect.h / 2)),
                meter.rect.w.saturating_sub(meter.rect.w / 2),
                meter.rect.h.saturating_sub(meter.rect.h / 2),
            ),
            meter.needle_colour,
            &meter.value.to_string(),
            clip,
        );
    }
}

fn draw_meter_ticks(
    fb: &mut Framebuffer,
    rect: Rect,
    start_angle: u8,
    end_angle: u8,
    number_of_ticks: u8,
    colour: Colour,
    clip: Rect,
) {
    if number_of_ticks == 0 {
        return;
    }
    let centre = ellipse_centre_point(rect);
    let denominator = u32::from(number_of_ticks.saturating_sub(1)).max(1);
    for tick in 0..number_of_ticks {
        let fraction = f64::from(tick) / f64::from(denominator);
        let angle = angle_for_fraction(start_angle, end_angle, false, fraction);
        let outer = ellipse_border_point(rect, angle);
        let inner = (
            centre
                .0
                .saturating_add((outer.0.saturating_sub(centre.0)) * 3 / 4),
            centre
                .1
                .saturating_add((outer.1.saturating_sub(centre.1)) * 3 / 4),
        );
        draw_line(
            fb,
            LineStroke {
                x0: inner.0,
                y0: inner.1,
                x1: outer.0,
                y1: outer.1,
                colour,
                width: 1,
                line_art: 0xFFFF,
            },
            clip,
        );
    }
}

#[derive(Debug, Clone, Copy)]
struct BarGraphDraw {
    rect: Rect,
    value: u32,
    target_value: u32,
    min: i32,
    max: i32,
    colour: Colour,
    target_line_colour: Colour,
    show_border: bool,
    show_target_line: bool,
    show_ticks: bool,
    number_of_ticks: u8,
    line_only: bool,
    arched: bool,
    horizontal: bool,
    direction_positive: bool,
    clockwise: bool,
    start_angle: u8,
    end_angle: u8,
    bar_width: u16,
}

fn draw_bar_graph(fb: &mut Framebuffer, bar: BarGraphDraw, clip: Rect) {
    if bar.rect.w == 0 || bar.rect.h == 0 {
        return;
    }
    let fraction = value_fraction(bar.value, bar.min, bar.max);
    if bar.arched {
        let width = bar.bar_width.max(1);
        if bar.show_border {
            let border = arc_points_for_fraction(
                bar.rect,
                bar.start_angle,
                bar.end_angle,
                bar.clockwise,
                1.0,
            );
            draw_polyline(fb, &border, bar.colour, width, 0xFFFF, clip);
        }
        if bar.line_only {
            draw_radial_bar_marker(
                fb,
                bar.rect,
                angle_for_fraction(bar.start_angle, bar.end_angle, bar.clockwise, fraction),
                bar.colour,
                width,
                clip,
            );
        } else {
            let points = arc_points_for_fraction(
                bar.rect,
                bar.start_angle,
                bar.end_angle,
                bar.clockwise,
                fraction,
            );
            draw_polyline(fb, &points, bar.colour, width, 0xFFFF, clip);
        }
        if bar.show_target_line {
            let target_fraction = value_fraction(bar.target_value, bar.min, bar.max);
            draw_radial_bar_marker(
                fb,
                bar.rect,
                angle_for_fraction(
                    bar.start_angle,
                    bar.end_angle,
                    bar.clockwise,
                    target_fraction,
                ),
                bar.target_line_colour,
                width,
                clip,
            );
        }
        return;
    }

    if bar.line_only {
        draw_linear_bar_marker(
            fb,
            bar.rect,
            bar.horizontal,
            bar.direction_positive,
            fraction,
            bar.colour,
            clip,
        );
    } else if bar.horizontal {
        let filled_w = scaled_extent(bar.rect.w, fraction);
        let x = if bar.direction_positive {
            bar.rect.x
        } else {
            bar.rect
                .x
                .saturating_add(i32::from(bar.rect.w.saturating_sub(filled_w)))
        };
        fill_rect(
            fb,
            Rect::new(x, bar.rect.y, filled_w, bar.rect.h),
            bar.colour,
            clip,
        );
    } else {
        let filled_h = scaled_extent(bar.rect.h, fraction);
        let y = if bar.direction_positive {
            bar.rect
                .y
                .saturating_add(i32::from(bar.rect.h.saturating_sub(filled_h)))
        } else {
            bar.rect.y
        };
        fill_rect(
            fb,
            Rect::new(bar.rect.x, y, bar.rect.w, filled_h),
            bar.colour,
            clip,
        );
    }
    if bar.show_ticks {
        draw_linear_bar_ticks(
            fb,
            bar.rect,
            bar.horizontal,
            bar.direction_positive,
            bar.number_of_ticks,
            bar.colour,
            clip,
        );
    }
    if bar.show_target_line {
        draw_linear_bar_marker(
            fb,
            bar.rect,
            bar.horizontal,
            bar.direction_positive,
            value_fraction(bar.target_value, bar.min, bar.max),
            bar.target_line_colour,
            clip,
        );
    }
    if bar.show_border {
        stroke_rect(fb, bar.rect, bar.colour, 1, 0xFFFF, 0, clip);
    }
}

fn draw_linear_bar_ticks(
    fb: &mut Framebuffer,
    rect: Rect,
    horizontal: bool,
    direction_positive: bool,
    ticks: u8,
    colour: Colour,
    clip: Rect,
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
        draw_linear_bar_marker(
            fb,
            rect,
            horizontal,
            direction_positive,
            fraction,
            colour,
            clip,
        );
    }
}

fn draw_linear_bar_marker(
    fb: &mut Framebuffer,
    rect: Rect,
    horizontal: bool,
    direction_positive: bool,
    fraction: f64,
    colour: Colour,
    clip: Rect,
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
        draw_line(
            fb,
            LineStroke {
                x0: x,
                y0: rect.y,
                x1: x,
                y1: rect.y.saturating_add(i32::from(rect.h.saturating_sub(1))),
                colour,
                width: 1,
                line_art: 0xFFFF,
            },
            clip,
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
        draw_line(
            fb,
            LineStroke {
                x0: rect.x,
                y0: y,
                x1: rect.x.saturating_add(i32::from(rect.w.saturating_sub(1))),
                y1: y,
                colour,
                width: 1,
                line_art: 0xFFFF,
            },
            clip,
        );
    }
}

fn draw_radial_bar_marker(
    fb: &mut Framebuffer,
    rect: Rect,
    angle: f64,
    colour: Colour,
    width: u16,
    clip: Rect,
) {
    let centre = ellipse_centre_point(rect);
    let outer = ellipse_border_point(rect, angle);
    let inner = (
        centre
            .0
            .saturating_add((outer.0.saturating_sub(centre.0)) / 2),
        centre
            .1
            .saturating_add((outer.1.saturating_sub(centre.1)) / 2),
    );
    draw_line(
        fb,
        LineStroke {
            x0: inner.0,
            y0: inner.1,
            x1: outer.0,
            y1: outer.1,
            colour,
            width,
            line_art: 0xFFFF,
        },
        clip,
    );
}

fn value_fraction(value: u32, min: i32, max: i32) -> f64 {
    if min >= max {
        return 0.0;
    }
    let min = i64::from(min);
    let max = i64::from(max);
    let range = max.saturating_sub(min);
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
    let end = angle_for_fraction(start_angle, end_angle, clockwise, fraction);
    let sweep = end - start;
    let steps = ((sweep.abs() / 5.0).ceil() as usize).clamp(2, 96);
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let angle = start + sweep * (step as f64 / steps as f64);
        let point = ellipse_border_point(rect, angle);
        if points.last().copied() != Some(point) {
            points.push(point);
        }
    }
    points
}

fn draw_thick_point(fb: &mut Framebuffer, x: i32, y: i32, width: u16, colour: Colour, clip: Rect) {
    if width == 0 {
        return;
    }
    let before = i32::from(width.saturating_sub(1) / 2);
    let after = i32::from(width / 2);
    for yy in -before..=after {
        for xx in -before..=after {
            fb.set_pixel(x.saturating_add(xx), y.saturating_add(yy), colour, clip);
        }
    }
}

fn draw_ellipse(fb: &mut Framebuffer, rect: Rect, style: ShapeDrawStyle, clip: Rect) {
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
                if style.filled {
                    fb.set_pixel(px, py, style.fill_colour, clip);
                }
                if style.line_enabled
                    && style.line_art == 0xFFFF
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
                    draw_thick_point(fb, px, py, style.line_width, style.line_colour, clip);
                }
            }
        }
    }
    if style.line_enabled && style.line_art != 0xFFFF {
        let points = ellipse_arc_points(rect, 0, 180);
        draw_polyline(
            fb,
            &points,
            style.line_colour,
            style.line_width,
            style.line_art,
            clip,
        );
    }
}

fn ellipse_contains(px: i32, py: i32, cx2: i64, cy2: i64, w: i64, h: i64, threshold: i64) -> bool {
    let dx2 = i64::from(px).saturating_mul(2).saturating_sub(cx2);
    let dy2 = i64::from(py).saturating_mul(2).saturating_sub(cy2);
    dx2.saturating_mul(dx2)
        .saturating_mul(h)
        .saturating_mul(h)
        .saturating_add(dy2.saturating_mul(dy2).saturating_mul(w).saturating_mul(w))
        <= threshold
}

struct EllipseArcDraw {
    rect: Rect,
    line_colour: Colour,
    fill_colour: Colour,
    filled: bool,
    line_enabled: bool,
    ellipse_type: u8,
    start_angle: u8,
    end_angle: u8,
    width: u16,
    line_art: u16,
}

#[derive(Debug, Clone, Copy)]
struct ShapeDrawStyle {
    line_colour: Colour,
    fill_colour: Colour,
    filled: bool,
    line_enabled: bool,
    line_width: u16,
    line_art: u16,
}

fn draw_ellipse_arc(fb: &mut Framebuffer, arc: EllipseArcDraw, clip: Rect) {
    if arc.rect.w == 0 || arc.rect.h == 0 {
        return;
    }
    let mut points = ellipse_arc_points(arc.rect, arc.start_angle, arc.end_angle);
    if points.len() < 2 {
        return;
    }

    match arc.ellipse_type {
        2 => {
            if arc.filled && points.len() >= 3 {
                draw_polygon(
                    fb,
                    &points,
                    ShapeDrawStyle {
                        line_colour: arc.line_colour,
                        fill_colour: arc.fill_colour,
                        filled: true,
                        line_enabled: false,
                        line_width: 0,
                        line_art: 0xFFFF,
                    },
                    clip,
                );
            }
            if arc.line_enabled {
                let first = points[0];
                points.push(first);
                draw_polyline(fb, &points, arc.line_colour, arc.width, arc.line_art, clip);
            }
        }
        3 => {
            let centre = ellipse_centre_point(arc.rect);
            let mut section = Vec::with_capacity(points.len() + 2);
            section.push(centre);
            section.extend(points.iter().copied());
            section.push(centre);
            if arc.filled && section.len() >= 3 {
                draw_polygon(
                    fb,
                    &section,
                    ShapeDrawStyle {
                        line_colour: arc.line_colour,
                        fill_colour: arc.fill_colour,
                        filled: true,
                        line_enabled: false,
                        line_width: 0,
                        line_art: 0xFFFF,
                    },
                    clip,
                );
            }
            if arc.line_enabled {
                draw_polyline(fb, &section, arc.line_colour, arc.width, arc.line_art, clip);
            }
        }
        _ => {
            if arc.line_enabled {
                draw_polyline(fb, &points, arc.line_colour, arc.width, arc.line_art, clip);
            }
        }
    }
}

fn draw_polyline(
    fb: &mut Framebuffer,
    points: &[(i32, i32)],
    colour: Colour,
    width: u16,
    line_art: u16,
    clip: Rect,
) {
    if width == 0 {
        return;
    }
    for pair in points.windows(2) {
        draw_line(
            fb,
            LineStroke {
                x0: pair[0].0,
                y0: pair[0].1,
                x1: pair[1].0,
                y1: pair[1].1,
                colour,
                width,
                line_art,
            },
            clip,
        );
    }
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
        let point = ellipse_border_point(rect, angle);
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

fn draw_polygon(fb: &mut Framebuffer, points: &[(i32, i32)], style: ShapeDrawStyle, clip: Rect) {
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
                if point_in_polygon(x, y, points) {
                    fb.set_pixel(x, y, style.fill_colour, clip);
                }
            }
        }
    }
    if style.line_enabled {
        for pair in points.windows(2) {
            draw_line(
                fb,
                LineStroke {
                    x0: pair[0].0,
                    y0: pair[0].1,
                    x1: pair[1].0,
                    y1: pair[1].1,
                    colour: style.line_colour,
                    width: style.line_width,
                    line_art: style.line_art,
                },
                clip,
            );
        }
    }
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

#[derive(Debug, Clone, Copy)]
struct TextCellStyle {
    foreground: Colour,
    background: Colour,
    metrics: FontMetrics,
    decoration: FontDecoration,
}

fn draw_text_cells(
    fb: &mut Framebuffer,
    rect: Rect,
    style: TextCellStyle,
    layout: &TextLayout,
    clip: Rect,
) {
    let cell_w = style.metrics.cell_w.max(1);
    let cell_h = style.metrics.cell_h.max(1);
    let glyph_w = cell_w.saturating_sub(1).max(1);
    let glyph_h = (cell_h.saturating_mul(2) / 3).max(1);
    let glyph_y_offset = i32::from(cell_h.saturating_sub(glyph_h) / 2);
    let (glyph_colour, cell_background) = if style.decoration.inverted {
        (style.background, Some(style.foreground))
    } else {
        (style.foreground, None)
    };
    for line in &layout.lines {
        for (col, ch) in line.text.chars().enumerate() {
            let x = rect.x.saturating_add(line.x_offset).saturating_add(
                i32::try_from(col)
                    .unwrap_or(i32::MAX)
                    .saturating_mul(i32::from(cell_w)),
            );
            let y = rect.y.saturating_add(line.y_offset);
            if let Some(background) = cell_background {
                fill_rect(fb, Rect::new(x, y, cell_w, cell_h), background, clip);
            }
            if !ch.is_whitespace() {
                for row in 0..glyph_h {
                    let skew = if style.decoration.italic {
                        glyph_h.saturating_sub(1).saturating_sub(row) / 3
                    } else {
                        0
                    };
                    let bold_extra = u16::from(style.decoration.bold && glyph_w < cell_w);
                    let draw_w = glyph_w
                        .saturating_add(bold_extra)
                        .min(cell_w.saturating_sub(skew).max(1));
                    fill_rect(
                        fb,
                        Rect::new(
                            x.saturating_add(i32::from(skew)),
                            y.saturating_add(glyph_y_offset)
                                .saturating_add(i32::from(row)),
                            draw_w,
                            1,
                        ),
                        glyph_colour,
                        clip,
                    );
                }
            }
            if style.decoration.underline {
                let underline_y = y.saturating_add(i32::from(cell_h.saturating_sub(1)));
                fill_rect(fb, Rect::new(x, underline_y, cell_w, 1), glyph_colour, clip);
            }
            if style.decoration.strikethrough {
                let strike_y = y.saturating_add(i32::from(cell_h / 2));
                fill_rect(fb, Rect::new(x, strike_y, cell_w, 1), glyph_colour, clip);
            }
        }
    }
}

fn draw_label_cells(fb: &mut Framebuffer, rect: Rect, colour: Colour, label: &str, clip: Rect) {
    for (col, ch) in label.chars().take(usize::from(rect.w / 6)).enumerate() {
        if ch.is_whitespace() {
            continue;
        }
        let x = rect
            .x
            .saturating_add(2)
            .saturating_add(i32::try_from(col).unwrap_or(i32::MAX).saturating_mul(6));
        let y = rect
            .y
            .saturating_add(i32::from(rect.h.saturating_sub(7)) / 2);
        fill_rect(fb, Rect::new(x, y, 4, 6), colour, clip);
    }
}

struct IndexedImageDraw<'a> {
    rect: Rect,
    source_width: u16,
    source_height: u16,
    format: u8,
    transparent: bool,
    transparency: u8,
    data: &'a [u8],
    palette: &'a Palette,
}

struct RgbaImageDraw<'a> {
    rect: Rect,
    source_width: u16,
    source_height: u16,
    data: &'a [u8],
}

struct UpdatedIndexedImageDraw<'a> {
    rect: Rect,
    update_width: u16,
    update_height: u16,
    update_format: u8,
    update_transparent_index: Option<u8>,
    update_data: &'a [u8],
    base_width: u16,
    base_height: u16,
    base_format: u8,
    base_transparent: bool,
    base_transparency: u8,
    base_data: &'a [u8],
    palette: &'a Palette,
}

struct PatternEllipseDraw<'a> {
    rect: Rect,
    anchor: (i32, i32),
    ellipse_type: u8,
    start_angle: u8,
    end_angle: u8,
    pattern: &'a FillPattern,
    palette: &'a Palette,
}

fn draw_pattern_rect(
    fb: &mut Framebuffer,
    rect: Rect,
    anchor: (i32, i32),
    pattern: &FillPattern,
    palette: &Palette,
    clip: Rect,
) {
    draw_pattern_pixels(fb, rect, anchor, pattern, palette, clip, |_, _| true);
}

fn draw_pattern_ellipse(fb: &mut Framebuffer, draw: PatternEllipseDraw<'_>, clip: Rect) {
    if draw.rect.w == 0 || draw.rect.h == 0 {
        return;
    }
    let w_i = i64::from(draw.rect.w);
    let h_i = i64::from(draw.rect.h);
    let threshold = w_i * w_i * h_i * h_i;
    let cx2 = i64::from(draw.rect.x)
        .saturating_mul(2)
        .saturating_add(i64::from(draw.rect.w.saturating_sub(1)));
    let cy2 = i64::from(draw.rect.y)
        .saturating_mul(2)
        .saturating_add(i64::from(draw.rect.h.saturating_sub(1)));
    if draw.ellipse_type == 0 {
        draw_pattern_pixels(
            fb,
            draw.rect,
            draw.anchor,
            draw.pattern,
            draw.palette,
            clip,
            |x, y| ellipse_contains(x, y, cx2, cy2, w_i, h_i, threshold),
        );
        return;
    }
    let mut points = ellipse_arc_points(draw.rect, draw.start_angle, draw.end_angle);
    match draw.ellipse_type {
        2 if points.len() >= 3 => {
            let first = points[0];
            points.push(first);
            draw_pattern_polygon(fb, &points, draw.anchor, draw.pattern, draw.palette, clip);
        }
        3 if points.len() >= 2 => {
            let centre = ellipse_centre_point(draw.rect);
            let mut section = Vec::with_capacity(points.len() + 2);
            section.push(centre);
            section.extend(points);
            section.push(centre);
            draw_pattern_polygon(fb, &section, draw.anchor, draw.pattern, draw.palette, clip);
        }
        _ => {}
    }
}

fn draw_pattern_polygon(
    fb: &mut Framebuffer,
    points: &[(i32, i32)],
    anchor: (i32, i32),
    pattern: &FillPattern,
    palette: &Palette,
    clip: Rect,
) {
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
    draw_pattern_pixels(
        fb,
        Rect::new(left, top, width, height),
        anchor,
        pattern,
        palette,
        clip,
        |x, y| point_in_polygon(x, y, points),
    );
}

fn draw_pattern_pixels(
    fb: &mut Framebuffer,
    rect: Rect,
    anchor: (i32, i32),
    pattern: &FillPattern,
    palette: &Palette,
    clip: Rect,
    contains: impl Fn(i32, i32) -> bool,
) {
    if rect.w == 0 || rect.h == 0 || pattern.width == 0 || pattern.height == 0 {
        return;
    }
    let Some(decoded) = pattern_data(pattern) else {
        return;
    };
    if !indexed_bitmap_has_minimum_len(&decoded, pattern.format, pattern.width, pattern.height) {
        return;
    }
    for yy in 0..rect.h {
        for xx in 0..rect.w {
            let x = rect.x.saturating_add(i32::from(xx));
            let y = rect.y.saturating_add(i32::from(yy));
            if !contains(x, y) {
                continue;
            }
            let pattern_x = pattern_axis_index(x, anchor.0, pattern.width);
            let pattern_y = pattern_axis_index(y, anchor.1, pattern.height);
            let Some(index) =
                indexed_bitmap_pixel(&decoded, pattern.format, pattern.width, pattern_x, pattern_y)
            else {
                return;
            };
            fb.set_pixel(x, y, palette.resolve(index), clip);
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
    decode_rle_pairs(&pattern.data)
}

fn decode_rle_pairs(data: &[u8]) -> Option<Vec<u8>> {
    if !data.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::new();
    for pair in data.chunks_exact(2) {
        let next_len = out.len().checked_add(usize::from(pair[0]))?;
        if next_len > MAX_FRAMEBUFFER_PIXELS {
            return None;
        }
        out.resize(next_len, pair[1]);
    }
    Some(out)
}

fn draw_indexed_image(fb: &mut Framebuffer, image: IndexedImageDraw<'_>, clip: Rect) {
    let IndexedImageDraw {
        rect,
        source_width,
        source_height,
        format,
        transparent,
        transparency,
        data,
        palette,
    } = image;
    if rect.w == 0 || rect.h == 0 || source_width == 0 || source_height == 0 {
        return;
    }
    if !indexed_bitmap_has_minimum_len(data, format, source_width, source_height) {
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
            let Some(index) = indexed_bitmap_pixel(data, format, source_width, source_x, source_y)
            else {
                return;
            };
            if !transparent || index != transparency {
                fb.set_pixel(
                    rect.x.saturating_add(i32::from(xx)),
                    rect.y.saturating_add(i32::from(yy)),
                    palette.resolve(index),
                    clip,
                );
            }
        }
    }
}

fn draw_rgba_image(fb: &mut Framebuffer, image: RgbaImageDraw<'_>, clip: Rect) {
    let RgbaImageDraw {
        rect,
        source_width,
        source_height,
        data,
    } = image;
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
            let alpha = pixel[3];
            if alpha == 0 {
                continue;
            }
            let colour = if alpha == 0xFF {
                Colour::rgb(pixel[0], pixel[1], pixel[2])
            } else {
                let x = rect.x.saturating_add(i32::from(xx));
                let y = rect.y.saturating_add(i32::from(yy));
                let dst = u16::try_from(x)
                    .ok()
                    .zip(u16::try_from(y).ok())
                    .and_then(|(x, y)| fb.pixel(x, y))
                    .unwrap_or_default();
                alpha_blend(Colour::rgb(pixel[0], pixel[1], pixel[2]), alpha, dst)
            };
            fb.set_pixel(
                rect.x.saturating_add(i32::from(xx)),
                rect.y.saturating_add(i32::from(yy)),
                colour,
                clip,
            );
        }
    }
}

fn alpha_blend(src: Colour, alpha: u8, dst: Colour) -> Colour {
    let alpha = u16::from(alpha);
    let inv = 255u16.saturating_sub(alpha);
    Colour::rgb(
        (((u16::from(src.r) * alpha) + (u16::from(dst.r) * inv) + 127) / 255) as u8,
        (((u16::from(src.g) * alpha) + (u16::from(dst.g) * inv) + 127) / 255) as u8,
        (((u16::from(src.b) * alpha) + (u16::from(dst.b) * inv) + 127) / 255) as u8,
    )
}

fn draw_updated_indexed_image(
    fb: &mut Framebuffer,
    image: UpdatedIndexedImageDraw<'_>,
    clip: Rect,
) {
    let UpdatedIndexedImageDraw {
        rect,
        update_width,
        update_height,
        update_format,
        update_transparent_index,
        update_data,
        base_width,
        base_height,
        base_format,
        base_transparent,
        base_transparency,
        base_data,
        palette,
    } = image;
    if rect.w == 0 || rect.h == 0 || update_width == 0 || update_height == 0 {
        return;
    }
    if !indexed_bitmap_has_minimum_len(update_data, update_format, update_width, update_height) {
        return;
    }
    let has_base = base_width != 0
        && base_height != 0
        && indexed_bitmap_has_minimum_len(base_data, base_format, base_width, base_height);

    for yy in 0..rect.h {
        for xx in 0..rect.w {
            let (base_x, base_y) = if has_base {
                (
                    u32::from(xx).saturating_mul(u32::from(base_width)) / u32::from(rect.w),
                    u32::from(yy).saturating_mul(u32::from(base_height)) / u32::from(rect.h),
                )
            } else {
                (
                    u32::from(xx).saturating_mul(u32::from(update_width)) / u32::from(rect.w),
                    u32::from(yy).saturating_mul(u32::from(update_height)) / u32::from(rect.h),
                )
            };

            let base_index = if has_base {
                let base_x =
                    usize::try_from(base_x.min(u32::from(base_width.saturating_sub(1))))
                        .unwrap_or(usize::MAX);
                let base_y =
                    usize::try_from(base_y.min(u32::from(base_height.saturating_sub(1))))
                        .unwrap_or(usize::MAX);
                indexed_bitmap_pixel(base_data, base_format, base_width, base_x, base_y)
            } else {
                None
            };

            let update_index = if base_x < u32::from(update_width) && base_y < u32::from(update_height) {
                let update_x = usize::try_from(base_x).unwrap_or(usize::MAX);
                let update_y = usize::try_from(base_y).unwrap_or(usize::MAX);
                indexed_bitmap_pixel(update_data, update_format, update_width, update_x, update_y)
            } else {
                None
            };

            let Some(index) = update_index
                .filter(|index| update_transparent_index != Some(*index))
                .filter(|index| indexed_bitmap_format_contains_index(base_format, *index))
                .or(base_index)
            else {
                continue;
            };

            if !base_transparent || index != base_transparency {
                fb.set_pixel(
                    rect.x.saturating_add(i32::from(xx)),
                    rect.y.saturating_add(i32::from(yy)),
                    palette.resolve(index),
                    clip,
                );
            }
        }
    }
}

fn indexed_bitmap_has_minimum_len(data: &[u8], format: u8, width: u16, height: u16) -> bool {
    indexed_bitmap_required_bytes(format, width, height).is_some_and(|required| data.len() >= required)
}

fn indexed_bitmap_required_bytes(format: u8, width: u16, height: u16) -> Option<usize> {
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

fn indexed_bitmap_pixel(
    data: &[u8],
    format: u8,
    width: u16,
    x: usize,
    y: usize,
) -> Option<u8> {
    let width = usize::from(width);
    match format {
        0 => {
            let row = width.saturating_add(7) / 8;
            let byte = *data.get(y.checked_mul(row)?.checked_add(x / 8)?)?;
            let shift = 7usize.saturating_sub(x % 8);
            Some((byte >> shift) & 0x01)
        }
        1 => {
            let row = width.saturating_add(1) / 2;
            let byte = *data.get(y.checked_mul(row)?.checked_add(x / 2)?)?;
            if x.is_multiple_of(2) {
                Some((byte >> 4) & 0x0F)
            } else {
                Some(byte & 0x0F)
            }
        }
        2 => data.get(y.checked_mul(width)?.checked_add(x)?).copied(),
        _ => None,
    }
}

const fn indexed_bitmap_format_contains_index(format: u8, index: u8) -> bool {
    match format {
        0 => index <= 1,
        1 => index <= 0x0F,
        2 => true,
        _ => false,
    }
}
