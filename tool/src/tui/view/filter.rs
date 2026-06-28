//! The "Filter" tab: an interactive editor for the capture filter.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Color;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::App;
use crate::tui::decode::FrameKind;
use crate::tui::filter::{Filter, FilterField};
use crate::tui::view::theme::{ACCENT, DIM, J1939, NMEA, TEXT, bold, dim, fg, panel};

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = panel("Filter");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let field_count = FilterField::ALL.len() as u16;
    let editing = app.editing.is_some();
    let edit_h: u16 = if editing { 3 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(field_count + 2),
            Constraint::Length(edit_h),
            Constraint::Min(0),
        ])
        .split(inner);

    draw_fields(frame, app, chunks[0]);
    if editing {
        draw_edit(frame, app, chunks[1]);
    }
    draw_kind_legend(frame, app, chunks[2]);
}

fn draw_fields(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![Line::from(Span::styled(
        "Press Enter to edit · x clears a field",
        dim(),
    ))];
    for field in FilterField::ALL {
        let selected = app.filter_field == field;
        let editing_now = app.editing == Some(field);
        let marker = if editing_now {
            "✎ "
        } else if selected {
            "▶ "
        } else {
            "  "
        };
        let value = app.filter.render(field);
        let display = if value.is_empty() {
            "(any)".to_string()
        } else {
            value
        };

        let mut spans = vec![Span::styled(marker, bold(ACCENT))];
        spans.push(Span::styled(
            format!("{:<11}", field.label()),
            bold(if selected { TEXT } else { DIM }),
        ));
        spans.push(Span::styled(format!("  {display}"), fg(ACCENT)));
        spans.push(Span::styled(format!("   · {}", field.hint()), dim()));
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_edit(frame: &mut Frame, app: &App, area: Rect) {
    let label = app.editing.map(FilterField::label).unwrap_or("input");
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(fg(ACCENT))
        .title(Line::from(format!(
            " edit: {label} (Enter=save Esc=cancel) "
        )));
    let inner = input_block.inner(area);
    frame.render_widget(input_block, area);

    let text = if app.input.is_empty() {
        Line::from(Span::styled("(empty)", dim()))
    } else {
        Line::from(Span::styled(app.input.clone(), fg(TEXT)))
    };
    frame.render_widget(Paragraph::new(text), inner);

    // Place a real terminal cursor inside the input.
    let cx = inner.x + (app.input_cursor as u16).min(inner.width.saturating_sub(1));
    let cy = inner.y;
    frame.set_cursor_position((cx, cy));
}

fn draw_kind_legend(frame: &mut Frame, app: &App, area: Rect) {
    let lines: Vec<Line> = vec![
        Line::from(Span::styled("Quick kind filters", bold(ACCENT))),
        legend_row("e", "extended-only", app.filter.extended_only),
        legend_row("s", "standard-only", app.filter.std_only),
        kind_row(
            "J",
            "J1939 / ISOBUS only",
            app.filter.kind,
            FrameKind::J1939,
            J1939,
        ),
        kind_row(
            "N",
            "NMEA 2000 only",
            app.filter.kind,
            FrameKind::Nmea2000,
            NMEA,
        ),
        legend_row("A", "all kinds (clear kind+ext/std)", false),
        Line::from(""),
        Line::from(Span::styled(
            "Tip: press / anywhere to jump straight to the data-text filter.",
            dim(),
        )),
    ];
    let _ = Filter::default(); // ensure type is reachable for docs
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn legend_row(key: &str, label: &str, on: bool) -> Line<'static> {
    let marker = if on { "●" } else { "○" };
    Line::from(vec![
        Span::styled(format!("  {key} "), bold(TEXT)),
        Span::styled(
            format!("{marker} {label}"),
            if on { bold(ACCENT) } else { dim() },
        ),
    ])
}

fn kind_row(
    key: &str,
    label: &str,
    current: Option<FrameKind>,
    this: FrameKind,
    color: Color,
) -> Line<'static> {
    let on = current == Some(this);
    let marker = if on { "●" } else { "○" };
    Line::from(vec![
        Span::styled(format!("  {key} "), bold(TEXT)),
        Span::styled(
            format!("{marker} {label}"),
            if on { bold(color) } else { dim() },
        ),
    ])
}
