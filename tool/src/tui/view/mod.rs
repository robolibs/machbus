//! Top-level rendering for `mechdump live`: a branded title bar, a custom
//! highlighted tab bar, the active tab body, and a status footer. Shared
//! styling lives in [`theme`].

pub mod filter;
pub mod help;
pub mod live;
pub mod nmea;
pub mod nodes;
pub mod pgn;
pub mod sniffer;
pub mod stats;
pub mod theme;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::tui::App;
use crate::tui::model::Tab;
use crate::tui::view::theme::{BG, DIM, GOLD, OK, TEXT, bold, dim, fg, meter_split};

/// Render the whole screen for one frame.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    // Paint the background so rounded panels sit on a consistent surface.
    frame.render_widget(Block::default().style(Style::default().bg(BG)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // title bar (branded)
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // body
            Constraint::Length(1), // status bar
        ])
        .split(area);

    draw_title_bar(frame, app, chunks[0]);
    draw_tabs(frame, app, chunks[1]);
    match app.tab {
        Tab::Live => live::render(frame, app, chunks[2]),
        Tab::Sniffer => sniffer::render(frame, app, chunks[2]),
        Tab::Pgn => pgn::render(frame, app, chunks[2], crate::tui::decode::FrameKind::J1939),
        Tab::Nmea => nmea::render(frame, app, chunks[2]),
        Tab::Nodes => nodes::render(frame, app, chunks[2]),
        Tab::Stats => stats::render(frame, app, chunks[2]),
        Tab::Filter => filter::render(frame, app, chunks[2]),
        Tab::Help => help::render(frame, app, chunks[2]),
    }
    draw_status_bar(frame, app, chunks[3]);
}

// ── title bar ───────────────────────────────────────────────────────────
fn draw_title_bar(frame: &mut Frame, app: &App, area: Rect) {
    let top = Rect { height: 1, ..area };
    let bot = Rect {
        y: area.y + 1,
        height: 1,
        ..area
    };

    // Line 1: brand chip + live indicators (right aligned).
    // NB: no BOLD here — bold+black renders as bright-black (slot 8) in many
    // terminal themes, so we keep it pure color-0 black on the accent.
    let brand = Span::styled(
        " ◆ MECHDUMP LIVE ",
        Style::default().fg(Color::Black).bg(theme::ACCENT),
    );
    let tag = Span::styled(" ISOBUS · J1939 · NMEA 2000 ", dim());

    // Right cluster: interface, status dot, rate, buffer meter.
    let iface = app_title_iface(app);
    let (dot, dot_col) = if app.paused {
        ("◼", GOLD)
    } else {
        ("●", OK)
    };
    let rate = app.stats.fps();
    let buf_fill = (app.frames.len() as f64 / app.buffer_cap as f64 * 100.0) as u64;
    let (buf_filled, buf_empty) = meter_split(app.frames.len() as u64, app.buffer_cap as u64, 10);

    let mut right = vec![
        Span::raw(" "),
        Span::styled(format!(" {iface} "), fg(TEXT)),
        Span::styled(format!("{dot} {rate:>5.0} f/s "), fg(dot_col)),
        Span::styled("▕", fg(DIM)),
        Span::styled(buf_filled, fg(theme::ACCENT)),
        Span::styled(buf_empty, fg(DIM)),
        Span::styled(format!(" {:>3}% ", buf_fill), dim()),
    ];
    if app.eof {
        right.push(Span::styled(" replay ◼ ", fg(GOLD)));
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(20), Constraint::Length(52)])
        .split(top);
    frame.render_widget(Paragraph::new(Line::from(vec![brand, tag])), cols[0]);
    frame.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        cols[1],
    );

    // Line 2: a thin separator in brand color.
    let bar = "━".repeat(area.width as usize);
    frame.render_widget(Paragraph::new(Span::styled(bar, fg(theme::BORDER))), bot);
}

fn app_title_iface(app: &App) -> String {
    app.frames
        .back()
        .map(|e| e.iface.as_str())
        .unwrap_or("—")
        .to_string()
}

// ── tab bar ─────────────────────────────────────────────────────────────
fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for (i, t) in Tab::ALL.iter().enumerate() {
        let active = *t == app.tab;
        let sep = if i > 0 {
            Span::styled(" │ ", fg(DIM))
        } else {
            Span::raw(" ")
        };
        spans.push(sep);
        let body = if active {
            Span::styled(
                format!(" {} {} ", t.hotkey(), t.title()),
                // Pure color-0 black on the accent — deliberately NO bold,
                // since bold+black renders as slot 8 (dark gray) on many
                // terminals and loses contrast on the red background.
                Style::default().fg(Color::Black).bg(theme::ACCENT),
            )
        } else {
            Span::styled(format!(" {} {} ", t.hotkey(), t.title()), fg(DIM))
        };
        spans.push(body);
    }
    spans.push(Span::raw(" "));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ── status bar ──────────────────────────────────────────────────────────
fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut left_spans: Vec<Span> = Vec::new();
    for (k, d) in [
        (" q", "quit"),
        (" p", "pause"),
        (" Tab", "next"),
        (" /", "filter"),
        (" d", "detail"),
        (" ?", "help"),
    ] {
        left_spans.push(Span::raw(" "));
        left_spans.push(Span::styled(k.to_string(), bold(theme::ACCENT)));
        left_spans.push(Span::styled(format!(" {d}"), dim()));
    }

    let right_spans = if let Some((msg, _)) = &app.message {
        vec![Span::styled(format!("❯ {msg} "), bold(GOLD))]
    } else {
        vec![
            Span::styled(format!(" {} frames ", app.stats.total), fg(TEXT)),
            Span::styled("·", fg(DIM)),
            Span::styled(format!(" {} shown ", app.frames.len()), fg(TEXT)),
            Span::styled("·", fg(DIM)),
            Span::styled(
                format!(" {} filter ", app.filter.active_count()),
                fg(theme::GOLD),
            ),
        ]
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(48)])
        .split(area);
    frame.render_widget(
        Paragraph::new(Line::from(left_spans)).style(Style::default().bg(BG)),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(right_spans))
            .alignment(Alignment::Right)
            .style(Style::default().bg(BG)),
        cols[1],
    );
}
