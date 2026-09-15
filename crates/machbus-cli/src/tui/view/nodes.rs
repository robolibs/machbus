//! The "Nodes" tab: the live J1939 address-claim table. Built from observed
//! PGN 60928 (Address Claimed) frames; the 8-byte NAME is decoded via
//! `machbus::net::Name` (manufacturer / function / identity / …).

use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row, Table, TableState};

use crate::tui::App;
use crate::tui::model::NodeEntry;
use crate::tui::view::theme::{ACCENT, EXT, PANEL, bold, dim, fg, panel};

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        "Nodes  ·  J1939 address claims  ·  {} claimed",
        app.nodes.rows.len(),
    );
    let block = panel(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let header = Row::new([
        Cell::from(Span::styled("Addr", bold(Color::Yellow))),
        Cell::from(Span::styled("NAME", bold(Color::Yellow))),
        Cell::from(Span::styled("Mfr", bold(Color::Yellow))),
        Cell::from(Span::styled("Func", bold(Color::Yellow))),
        Cell::from(Span::styled("Class", bold(Color::Yellow))),
        Cell::from(Span::styled("Identity", bold(Color::Yellow))),
        Cell::from(Span::styled("IG", bold(Color::Yellow))),
        Cell::from(Span::styled("SC", bold(Color::Yellow))),
        Cell::from(Span::styled("Claims", bold(Color::Yellow))),
    ])
    .height(1);

    let mut rows: Vec<&NodeEntry> = app.nodes.rows.values().collect();
    rows.sort_by_key(|e| e.address);

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|e| {
            let n = &e.name;
            Row::new([
                Cell::from(Span::styled(format!("{:02X}", e.address), bold(EXT))),
                Cell::from(Span::styled(format!("{:016X}", n.raw), dim())),
                Cell::from(Span::styled(
                    format!("{}", n.manufacturer_code()),
                    fg(Color::White),
                )),
                Cell::from(Span::styled(
                    format!("{}", n.function_code()),
                    fg(Color::White),
                )),
                Cell::from(Span::styled(
                    format!("{}", n.device_class()),
                    fg(Color::White),
                )),
                Cell::from(Span::styled(format!("{}", n.identity_number()), fg(ACCENT))),
                Cell::from(Span::styled(format!("{}", n.industry_group()), dim())),
                Cell::from(Span::styled(
                    if n.self_configurable() { "Y" } else { "-" },
                    bold(if n.self_configurable() {
                        ACCENT
                    } else {
                        Color::DarkGray
                    }),
                )),
                Cell::from(Span::styled(format!("{}", e.count), dim())),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Length(18),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(10),
        Constraint::Length(4),
        Constraint::Length(4),
        Constraint::Length(8),
    ];

    let mut state = TableState::default();
    if !rows.is_empty() {
        let sel = app.selected.min(rows.len() - 1);
        state.select(Some(sel));
        app.selected = sel;
    }

    if rows.is_empty() {
        frame.render_widget(
            ratatui::widgets::Paragraph::new(Span::styled(
                "  no Address Claimed (PGN 60928) frames seen yet",
                dim(),
            )),
            inner,
        );
        return;
    }

    let table = Table::new(table_rows, widths)
        .header(header)
        .row_highlight_style(Style::default().bg(PANEL).add_modifier(Modifier::BOLD));
    frame.render_stateful_widget(table, inner, &mut state);
}
