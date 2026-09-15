//! Layout for `machbus term`: title bar, framed VT screen (painted by
//! [`crate::term::fbview`]), an optional mask-selector panel, and a status
//! line. Chrome uses the terminal ANSI palette; the VT screen itself is
//! truecolour pixels.

use machbus::isobus::vt::{ObjectPool, ObjectType};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::term::TermApp;
use crate::term::fbview;

// ANSI-palette chrome (terminal-theme aware).
const ACCENT: Color = Color::Cyan;
const GOLD: Color = Color::Yellow;
const DIM: Color = Color::DarkGray;
const TEXT: Color = Color::White;

pub fn render(frame: &mut Frame, app: &mut TermApp) {
    app.message_expired();
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // title
            Constraint::Min(0),    // body
            Constraint::Length(1), // status
        ])
        .split(area);

    draw_title(frame, app, chunks[0]);
    draw_body(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
}

fn draw_title(frame: &mut Frame, app: &TermApp, area: Rect) {
    let brand = Span::styled(
        " ◆ machbus term ",
        Style::default().fg(Color::Black).bg(ACCENT),
    );
    let sub = Span::styled(" ISOBUS Virtual Terminal ", Style::default().fg(DIM));
    let active = app.doc.scene().active_mask.raw();
    let right = Line::from(vec![
        Span::styled(format!(" {} ", app.source), Style::default().fg(TEXT)),
        Span::styled("·", Style::default().fg(DIM)),
        Span::styled(
            format!(" mask 0x{:04X} ", active),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        ),
        Span::styled("·", Style::default().fg(DIM)),
        Span::styled(
            format!(" {}×{} px ", app.frame.width(), app.frame.height()),
            Style::default().fg(TEXT),
        ),
    ])
    .alignment(Alignment::Right);

    let top = Rect { height: 1, ..area };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(56)])
        .split(top);
    frame.render_widget(Paragraph::new(Line::from(vec![brand, sub])), cols[0]);
    frame.render_widget(Paragraph::new(right), cols[1]);

    let bar = "━".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Span::styled(bar, Style::default().fg(DIM))),
        Rect {
            y: area.y + 1,
            height: 1,
            ..area
        },
    );
}

fn draw_body(frame: &mut Frame, app: &mut TermApp, area: Rect) {
    if app.panel_open {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(30)])
            .split(area);
        draw_screen(frame, app, cols[0]);
        draw_mask_panel(frame, app, cols[1]);
    } else {
        draw_screen(frame, app, area);
    }
}

fn draw_screen(frame: &mut Frame, app: &TermApp, area: Rect) {
    let title = format!(
        "VT screen  ·  {}×{}  ·  half-block",
        app.frame.width(),
        app.frame.height(),
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(DIM))
        .title(
            Line::from(format!(" {title} "))
                .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // Paint the framebuffer directly into the ratatui buffer.
    fbview::paint(frame.buffer_mut(), inner, &app.frame);
}

fn draw_mask_panel(frame: &mut Frame, app: &TermApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(DIM))
        .title(
            Line::from(format!(" Masks ({}) ", app.masks.len()))
                .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = app
        .masks
        .iter()
        .map(|id| {
            let kind = mask_kind(app.doc.pool(), *id);
            ListItem::new(Line::from(vec![
                Span::styled(format!(" 0x{:04X} ", id.raw()), Style::default().fg(GOLD)),
                Span::styled(kind.to_string(), Style::default().fg(TEXT)),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !app.masks.is_empty() {
        state.select(Some(app.mask_sel.min(app.masks.len() - 1)));
    }
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, inner, &mut state);
}

fn draw_status(frame: &mut Frame, app: &TermApp, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();
    for (k, d) in [
        ("q/Esc", "quit"),
        ("Tab", "panel"),
        ("j/k ↑↓", "mask"),
        ("n/p", "next/prev"),
        ("r", "re-render"),
    ] {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            k.to_string(),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(format!(" {d}"), Style::default().fg(DIM)));
        spans.push(Span::raw(" "));
    }
    let left = Line::from(spans);

    let right_spans = if let Some((msg, _)) = &app.message {
        vec![Span::styled(
            format!("❯ {msg} "),
            Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![
            Span::styled(
                format!(" {} objects ", app.doc.pool().size()),
                Style::default().fg(TEXT),
            ),
            Span::styled("·", Style::default().fg(DIM)),
            Span::styled(
                format!(" {}/{} ", app.mask_sel.saturating_add(1), app.masks.len()),
                Style::default().fg(TEXT),
            ),
        ]
    };
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(40)])
        .split(area);
    frame.render_widget(Paragraph::new(left), cols[0]);
    frame.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        cols[1],
    );
}

/// Short human label for a mask object's type, looked up by ID.
fn mask_kind(pool: &ObjectPool, id: machbus::isobus::vt::ObjectID) -> &'static str {
    let ty = pool.objects().iter().find(|o| o.id == id).map(|o| o.r#type);
    match ty {
        Some(ObjectType::DataMask) => "Data",
        Some(ObjectType::AlarmMask) => "Alarm",
        Some(ObjectType::WindowMask) => "Window",
        _ => "Mask",
    }
}
