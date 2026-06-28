//! The "PGN" tab: an aggregate table of J1939 / ISOBUS PGNs seen on the bus,
//! sorted by frequency. The same table renderer is reused by the NMEA tab
//! with a different kind filter.

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row, Table, TableState};

use crate::tui::App;
use crate::tui::decode::FrameKind;
use crate::tui::model::PgnStat;
use crate::tui::view::theme::{J1939, NMEA, PANEL, bold, dim, fg, panel};

/// Render the J1939 / ISOBUS PGN aggregate.
pub fn render(frame: &mut Frame, app: &mut App, area: Rect, kind: FrameKind) {
    let rows = build_rows(app, kind);
    let title = format!(
        "{} PGNs  ·  {} distinct  ·  Enter filters this PGN in Live",
        kind_label(kind),
        rows.len(),
    );
    draw_table(frame, app, area, kind, &rows, &title);
    app.pgn_rows = rows;
}

/// Build the sorted PGN list for a given kind (count desc, then PGN asc).
pub(crate) fn build_rows(app: &App, kind: FrameKind) -> Vec<u32> {
    let mut entries: Vec<&PgnStat> = app
        .stats
        .per_pgn
        .values()
        .filter(|s| s.kind == kind)
        .collect();
    entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.pgn.cmp(&b.pgn)));
    entries.iter().map(|e| e.pgn).collect()
}

pub(crate) fn kind_label(kind: FrameKind) -> &'static str {
    match kind {
        FrameKind::J1939 => "J1939/ISOBUS",
        FrameKind::Nmea2000 => "NMEA 2000",
        FrameKind::Raw => "Raw CAN",
    }
}

fn draw_table(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    kind: FrameKind,
    rows: &[u32],
    title: &str,
) {
    draw_table_inner(frame, app, area, kind, rows, title);
}

/// Public entry point reused by the NMEA tab.
pub(crate) fn draw_nmea_table(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    rows: &[u32],
    title: &str,
) {
    draw_table_inner(frame, app, area, FrameKind::Nmea2000, rows, title);
}

fn draw_table_inner(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
    kind: FrameKind,
    rows: &[u32],
    title: &str,
) {
    let block = panel(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header_cells = [
        Cell::from(Span::styled("PGN", bold(Color::Yellow))),
        Cell::from(Span::styled("Name", bold(Color::Yellow))),
        Cell::from(Span::styled("Count", bold(Color::Yellow))),
        Cell::from(Span::styled("Src", bold(Color::Yellow))),
        Cell::from(Span::styled("Last data", bold(Color::Yellow))),
    ];
    let header = Row::new(header_cells).height(1).bottom_margin(0);

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|pgn| {
            let stat = app.stats.per_pgn.get(pgn);
            let count = stat.map_or(0, |s| s.count);
            let name = stat.and_then(|s| s.name).unwrap_or("unknown");
            let src = stat.map_or(0, |s| s.last_src);
            let data = stat.map_or_else(String::new, |s| hex(&s.last_data, s.last_dlc));
            let accent = if kind == FrameKind::Nmea2000 {
                NMEA
            } else {
                J1939
            };
            Row::new(vec![
                Cell::from(Span::styled(format!("{pgn:05X}"), bold(accent))),
                Cell::from(Span::styled(name.to_string(), fg(Color::White))),
                Cell::from(Span::styled(count.to_string(), fg(Color::Cyan))),
                Cell::from(Span::styled(format!("{src:02X}"), fg(Color::White))),
                Cell::from(Span::styled(data, dim())),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Min(16),
        Constraint::Length(8),
        Constraint::Length(5),
        Constraint::Min(20),
    ];
    let mut state = TableState::default();
    if !rows.is_empty() {
        let sel = app.selected.min(rows.len() - 1);
        state.select(Some(sel));
        app.selected = sel;
    }
    let table = Table::new(table_rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(PANEL).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");
    frame.render_stateful_widget(table, inner, &mut state);
}

fn hex(data: &[u8; 8], dlc: u8) -> String {
    let n = (dlc as usize).min(8);
    let mut s = String::with_capacity(n * 3);
    for (i, b) in data[..n].iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{b:02X}"));
    }
    s
}
