//! Rendering for `machbus drive`: keyboard + joystick + compact telemetry.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use machbus::session::Session;
use machbus::session::plugins::{Gnss, Guidance};

use super::DriveState;
use super::joystick::PadState;
use super::keyboard::KeyboardState;

const CYAN: Color = Color::Cyan;
const GOLD: Color = Color::Yellow;
const GREEN: Color = Color::Green;
const RED: Color = Color::Red;
const GRAY: Color = Color::DarkGray;
const WHITE: Color = Color::White;

pub fn render_keyboard(f: &mut Frame, state: &DriveState, kb: &KeyboardState, session: &Session) {
    let area = f.area();
    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // title
            Constraint::Min(0),    // keyboard (takes the bulk)
            Constraint::Length(8), // telemetry (6 content + 2 border)
            Constraint::Length(1), // status
        ])
        .split(area);

    draw_title(f, state, cols[0]);
    draw_keyboard(f, state, kb, cols[1]);
    draw_telemetry(f, state, session, cols[2]);
    draw_status(f, state, cols[3]);
}

// ─── gamepad layout ─────────────────────────────────────────────────────

fn draw_gamepad(f: &mut Frame, state: &DriveState, pad: &PadState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(GRAY))
        .title(Span::styled(
            format!(
                " {} ",
                if pad.pad_name.is_empty() {
                    "Gamepad"
                } else {
                    &pad.pad_name
                }
            ),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 14 {
        return;
    }

    let cx = inner.x + inner.width / 2;
    let cy = inner.y + inner.height / 2;

    // Left stick circle — bigger: 14 wide × 7 tall.
    draw_stick(f, cx - 22, cy - 4, pad.lstick_x, pad.lstick_y, "L", GOLD);
    // Right stick circle — same size.
    draw_stick(f, cx + 12, cy - 4, 0.0, 0.0, "R", GRAY);

    // Trigger bars above.
    let ty = inner.y + 1;
    let engaged = pad.rtrigger > 0.3;
    draw_trigger(f, cx - 26, ty, "L2", pad.ltrigger, GRAY);
    let r2_col = if engaged { GREEN } else { RED };
    let r2_label = if engaged { "R2 ENGAGED" } else { "R2 HOLD" };
    draw_trigger(f, cx + 14, ty, r2_label, pad.rtrigger, r2_col);

    // Buttons (A/B/X/Y) in a diamond — bigger spacing.
    let bx = cx + 18;
    let by = cy;
    draw_pad_btn(f, bx, by - 3, "Y", pad.y_held);
    draw_pad_btn(f, bx - 6, by, "X", pad.x_held);
    draw_pad_btn(f, bx + 6, by, "B", pad.b_held);
    draw_pad_btn(f, bx, by + 3, "A", pad.a_held);

    // D-pad on the left — bigger spacing.
    let dx = cx - 18;
    draw_pad_btn(f, dx, by - 3, "↑", pad.dpad_up);
    draw_pad_btn(f, dx - 6, by, "←", false);
    draw_pad_btn(f, dx + 6, by, "→", false);
    draw_pad_btn(f, dx, by + 3, "↓", pad.dpad_down);

    // Start button.
    draw_pad_btn(f, cx, by, "⌖", pad.start_held);

    // Labels under left stick.
    f.render_widget(
        Paragraph::new("steer / throttle")
            .style(Style::default().fg(GRAY))
            .alignment(Alignment::Center),
        Rect { x: cx - 28, y: cy + 4, width: 16, height: 1 },
    );

    // Info line at the bottom.
    f.render_widget(
        Paragraph::new(format!(
            " L: ({:+.2},{:+.2})  R2:{:.0}%  L2:{:.0}%  {}× counter",
            pad.lstick_x, pad.lstick_y,
            pad.rtrigger * 100.0, pad.ltrigger * 100.0,
            state.counter_mult,
        ))
        .style(Style::default().fg(WHITE)),
        Rect { x: inner.x + 1, y: inner.y + inner.height.saturating_sub(1), width: inner.width - 2, height: 1 },
    );
}

fn draw_stick(f: &mut Frame, x: u16, y: u16, sx: f64, sy: f64, label: &str, col: Color) {
    let w = 14u16;
    let h = 7u16;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(col))
        .title(Span::styled(format!(" {label} "), Style::default().fg(col)));
    let inner = block.inner(Rect { x, y, width: w, height: h });
    f.render_widget(block, Rect { x, y, width: w, height: h });

    let ccx = inner.x + inner.width / 2;
    let mid_y = inner.y + inner.height / 2;

    // Dot position (scaled into the inner area).
    let dot_x = inner.x + ((sx + 1.0) * 0.5 * (inner.width as f64 - 1.0)).round() as u16;
    let dot_y = inner.y + ((1.0 - (sy + 1.0) * 0.5) * (inner.height as f64 - 1.0)).round() as u16;

    // Crosshair — horizontal line through centre.
    let h_line: String = (0..inner.width)
        .map(|i| if inner.x + i == ccx { '┼' } else { '─' })
        .collect();
    f.render_widget(
        Paragraph::new(Span::styled(h_line, Style::default().fg(GRAY))),
        Rect { x: inner.x, y: mid_y, width: inner.width, height: 1 },
    );

    // Crosshair — vertical line through centre.
    for ry in inner.y..inner.y + inner.height {
        let ch = if ry == mid_y { '┼' } else { '│' };
        f.render_widget(
            Paragraph::new(Span::styled(ch.to_string(), Style::default().fg(GRAY))),
            Rect { x: ccx, y: ry, width: 1, height: 1 },
        );
    }

    // The dot.
    f.render_widget(
        Paragraph::new(Span::styled("●", Style::default().fg(col).add_modifier(Modifier::BOLD))),
        Rect { x: dot_x, y: dot_y, width: 1, height: 1 },
    );
}

fn draw_trigger(f: &mut Frame, x: u16, y: u16, label: &str, value: f64, col: Color) {
    let w = 10u16;
    let pct = (value.clamp(0.0, 1.0) * w as f64).round() as u16;
    let bar: String = "▰".repeat(pct as usize) + &"▱".repeat((w - pct) as usize);
    let line = format!("{label} {bar}");
    f.render_widget(
        Paragraph::new(Span::styled(line, Style::default().fg(col))),
        Rect {
            x,
            y,
            width: w + 4,
            height: 1,
        },
    );
}

fn draw_pad_btn(f: &mut Frame, x: u16, y: u16, label: &str, held: bool) {
    let style = if held {
        Style::default()
            .fg(Color::Black)
            .bg(CYAN)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(WHITE).bg(Color::Indexed(236))
    };
    f.render_widget(
        Paragraph::new(format!(" {label} "))
            .style(style)
            .alignment(Alignment::Center),
        Rect {
            x,
            y,
            width: label.chars().count() as u16 + 2,
            height: 1,
        },
    );
}

pub fn render_joystick(f: &mut Frame, state: &DriveState, pad: &PadState, session: &Session) {
    let area = f.area();
    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(0),
            Constraint::Length(8),
            Constraint::Length(1),
        ])
        .split(area);

    draw_title(f, state, cols[0]);
    draw_gamepad(f, state, pad, cols[1]);
    draw_telemetry(f, state, session, cols[2]);
    draw_status(f, state, cols[3]);
}

// ─── title ───────────────────────────────────────────────────────────────

fn draw_title(f: &mut Frame, state: &DriveState, area: Rect) {
    let brand = Span::styled(
        " machbus drive ",
        Style::default()
            .fg(Color::Black)
            .bg(CYAN)
            .add_modifier(Modifier::BOLD),
    );
    let sub = Span::styled(" ISOBUS guidance ", Style::default().fg(GRAY));
    let claim_dot = if state.claimed { "●" } else { "○" };
    let claim_col = if state.claimed { GREEN } else { RED };
    let claim = Span::styled(
        format!(" {claim_dot} 0x{:02X} ", state.claimed_addr.max(0x80)),
        Style::default().fg(claim_col).add_modifier(Modifier::BOLD),
    );
    let right = Line::from(claim).alignment(Alignment::Right);
    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(20)])
        .split(area);
    f.render_widget(Paragraph::new(Line::from(vec![brand, sub])), top[0]);
    f.render_widget(Paragraph::new(right), top[1]);
}

// ─── keyboard ────────────────────────────────────────────────────────────

fn draw_keyboard(f: &mut Frame, state: &DriveState, kb: &KeyboardState, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(GRAY))
        .title(Span::styled(
            " Controls ",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let cx = inner.x + inner.width / 2;
    let mut y = inner.y;

    // Row spacing: 4 rows per key (3 cap + 1 hint).
    let step = 4u16;

    // W
    key(f, cx, y, 'W', kb.kw.lit(), "forward");
    y += step;
    // A S D
    key(f, cx - 14, y, 'A', kb.ka.lit(), "left");
    key(f, cx, y, 'S', kb.ks.lit(), "brake");
    key(f, cx + 14, y, 'D', kb.kd.lit(), "right");
    y += step;
    // ENTER
    key_wide(f, cx, y, "ENTER", 14, kb.kenter.lit(), "stop");
    y += step;
    // I K
    key(f, cx - 7, y, 'I', kb.ki.lit(), "limit +");
    key(f, cx + 7, y, 'K', kb.kk.lit(), "limit −");
    y += step;
    // H J
    key(f, cx - 7, y, 'H', kb.kh.lit(), "hitch ↑");
    key(f, cx + 7, y, 'J', kb.kj.lit(), "hitch ↓");
    y += step;
    // P O
    key(f, cx - 7, y, 'P', kb.kp.lit(), "PTO on");
    key(f, cx + 7, y, 'O', kb.ko.lit(), "PTO off");
    y += step;
    // X
    key_fmt(
        f,
        cx,
        y,
        &format!("X×{}", state.counter_mult),
        9,
        kb.kx.lit(),
        "counter rate",
    );
}

fn key(f: &mut Frame, cx: u16, y: u16, ch: char, held: bool, hint: &str) {
    let w = 7u16;
    let x = cx.saturating_sub(w / 2);
    key_box(f, x, y, w, &format!("  {ch}  "), held);
    f.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(GRAY))
            .alignment(Alignment::Center),
        Rect {
            x: cx.saturating_sub(10),
            y: y + 3,
            width: 21,
            height: 1,
        },
    );
}

fn key_wide(f: &mut Frame, cx: u16, y: u16, label: &str, w: u16, held: bool, hint: &str) {
    let x = cx.saturating_sub(w / 2);
    key_box(f, x, y, w, &format!(" {} ", label), held);
    f.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(GRAY))
            .alignment(Alignment::Center),
        Rect {
            x: cx.saturating_sub(10),
            y: y + 3,
            width: 21,
            height: 1,
        },
    );
}

fn key_fmt(f: &mut Frame, cx: u16, y: u16, label: &str, w: u16, held: bool, hint: &str) {
    let x = cx.saturating_sub(w / 2);
    key_box(f, x, y, w, &format!("{}  ", label), held);
    f.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(GRAY))
            .alignment(Alignment::Center),
        Rect {
            x: cx.saturating_sub(10),
            y: y + 3,
            width: 21,
            height: 1,
        },
    );
}

fn key_box(f: &mut Frame, x: u16, y: u16, w: u16, label: &str, held: bool) {
    if w < 3 || y + 2 > f.area().bottom() {
        return;
    }
    let (fg_c, bg_c, border_c) = if held {
        (Color::Black, CYAN, CYAN)
    } else {
        (WHITE, Color::Reset, GRAY)
    };
    let corners = if held { "╔╗╚╝" } else { "╭╮╰╯" };
    let h = if held { "═" } else { "─" };
    let v = if held { "║" } else { "│" };
    let b_style = Style::default().fg(border_c);
    let i_style = Style::default()
        .fg(fg_c)
        .bg(bg_c)
        .add_modifier(Modifier::BOLD);

    // Top border
    let mut top = String::new();
    top.push(corners.chars().next().unwrap());
    for _ in 0..w - 2 {
        top.push_str(h);
    }
    top.push(corners.chars().nth(1).unwrap());
    f.render_widget(
        Paragraph::new(Span::styled(top, b_style)),
        Rect {
            x,
            y,
            width: w,
            height: 1,
        },
    );
    // Middle
    f.render_widget(
        Paragraph::new(Span::styled(label.to_string(), i_style)).alignment(Alignment::Center),
        Rect {
            x: x + 1,
            y: y + 1,
            width: w - 2,
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::styled(v.to_string(), b_style)),
        Rect {
            x,
            y: y + 1,
            width: 1,
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::styled(v.to_string(), b_style)),
        Rect {
            x: x + w - 1,
            y: y + 1,
            width: 1,
            height: 1,
        },
    );
    // Bottom border
    let mut bot = String::new();
    bot.push(corners.chars().nth(2).unwrap());
    for _ in 0..w - 2 {
        bot.push_str(h);
    }
    bot.push(corners.chars().nth(3).unwrap());
    f.render_widget(
        Paragraph::new(Span::styled(bot, b_style)),
        Rect {
            x,
            y: y + 2,
            width: w,
            height: 1,
        },
    );
}

// ─── telemetry (6 compact lines) ─────────────────────────────────────────

fn draw_telemetry(f: &mut Frame, state: &DriveState, session: &Session, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(GRAY))
        .title(Span::styled(
            " Telemetry ",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // 6 lines max. Values + bars on the SAME line.
    let lines: Vec<Line> = vec![
        // 1: Speed + bar
        line_bar(
            "spd",
            &format!("{:.1} km/h", state.speed * 3.6),
            state.speed / state.speed_limit.abs().max(0.5),
            GOLD,
            inner.width,
        ),
        // 2: Curvature + bar
        line_bar(
            "crv",
            &format!("{:+.1}/km", state.curvature()),
            state.curvature() / state.max_curvature.abs().max(1.0),
            GOLD,
            inner.width,
        ),
        // 3: Steer + bar
        line_bar_c(
            "str",
            &format!("{:+.2}", state.steer),
            state.steer,
            inner.width,
        ),
        // 4: Steering readiness + est curvature
        {
            let mut spans = vec![Span::styled("str ", Style::default().fg(GRAY))];
            if let Some(g) = session.get::<Guidance>() {
                let ready = g.is_steering_ready();
                spans.push(Span::styled(
                    if ready { "●READY" } else { "○OFFLINE" },
                    Style::default()
                        .fg(if ready { GREEN } else { RED })
                        .add_modifier(Modifier::BOLD),
                ));
                if let Some(est) = g.estimated_curvature() {
                    spans.push(Span::styled(
                        format!("  est={:.1}", est),
                        Style::default().fg(WHITE),
                    ));
                }
            } else {
                spans.push(Span::styled("—", Style::default().fg(GRAY)));
            }
            spans.push(Span::styled(
                format!("  │  {}× counter", state.counter_mult),
                Style::default().fg(CYAN),
            ));
            Line::from(spans)
        },
        // 5: GNSS position
        {
            let mut spans = vec![Span::styled("gnss ", Style::default().fg(GRAY))];
            if let Some(gnss) = session.get::<Gnss>()
                && let Some(pos) = gnss.latest_position()
            {
                spans.push(Span::styled(
                    format!("{:.5} {:.5}", pos.wgs.latitude, pos.wgs.longitude),
                    Style::default().fg(WHITE),
                ));
                if let Some(h) = pos.heading_rad {
                    spans.push(Span::styled(
                        format!("  hdg {:.0}°", h.to_degrees()),
                        Style::default().fg(WHITE),
                    ));
                }
            } else {
                spans.push(Span::styled("—", Style::default().fg(GRAY)));
            }
            Line::from(spans)
        },
        // 6: GNSS speed + limit
        {
            let mut spans = vec![Span::styled("lim ", Style::default().fg(GRAY))];
            spans.push(Span::styled(
                format!("{:.1} m/s", state.speed_limit),
                Style::default().fg(CYAN),
            ));
            if let Some(gnss) = session.get::<Gnss>()
                && let Some(pos) = gnss.latest_position()
                && let Some(v) = pos.speed_mps
            {
                spans.push(Span::styled(
                    format!("  │  gnss={:.1} km/h", v * 3.6),
                    Style::default().fg(WHITE),
                ));
            }
            Line::from(spans)
        },
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(WHITE)),
        inner,
    );
}

/// A label + value + bar on ONE line. Bar fills left-to-right for 0..1.
fn line_bar(label: &str, value: &str, ratio: f64, col: Color, width: u16) -> Line<'static> {
    let ratio = ratio.clamp(0.0, 1.0);
    let prefix_len = (label.len() + 1 + value.len() + 2) as u16;
    let bar_width = width.saturating_sub(prefix_len + 2) as usize;
    let filled = (ratio * bar_width as f64).round() as usize;
    let bar: String =
        "▰".repeat(filled.min(bar_width)) + &"▱".repeat(bar_width - filled.min(bar_width));
    Line::from(vec![
        Span::styled(format!("{label} "), Style::default().fg(GRAY)),
        Span::styled(
            format!("{value}  "),
            Style::default().fg(col).add_modifier(Modifier::BOLD),
        ),
        Span::styled(bar, Style::default().fg(CYAN)),
    ])
}

/// A label + value + centred bar for -1..1 values.
fn line_bar_c(label: &str, value: &str, ratio: f64, width: u16) -> Line<'static> {
    let ratio = ratio.clamp(-1.0, 1.0);
    let prefix_len = (label.len() + 1 + value.len() + 2) as u16;
    let bar_width = width.saturating_sub(prefix_len + 2) as usize;
    let half = bar_width / 2;
    let abs = (ratio.abs() * half as f64).round() as usize;
    let (l_fill, r_fill) = if ratio >= 0.0 { (0, abs) } else { (abs, 0) };
    let left = "▰".repeat(l_fill.min(half));
    let left_e = "▱".repeat(half - l_fill.min(half));
    let right = "▰".repeat(r_fill.min(half));
    let right_e = "▱".repeat(half - r_fill.min(half));
    Line::from(vec![
        Span::styled(format!("{label} "), Style::default().fg(GRAY)),
        Span::styled(
            format!("{value}  "),
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{left_e}{left}"),
            Style::default().fg(if ratio < 0.0 { GOLD } else { GRAY }),
        ),
        Span::styled("│", Style::default().fg(GRAY)),
        Span::styled(
            format!("{right}{right_e}"),
            Style::default().fg(if ratio > 0.0 { GOLD } else { GRAY }),
        ),
    ])
}

// ─── status bar ──────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, state: &DriveState, area: Rect) {
    let mut spans = Vec::new();
    for (k, d) in [
        ("q", "quit"),
        ("WASD", "drive"),
        ("⏎", "stop"),
        ("I/K", "spd"),
        ("X", "ctr"),
        ("H/J", "hitch"),
        ("P/O", "PTO"),
    ] {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(d.to_string(), Style::default().fg(GRAY)));
        spans.push(Span::raw("  "));
    }
    spans.push(Span::styled(
        format!("❯ {}", state.status),
        Style::default().fg(GOLD),
    ));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
