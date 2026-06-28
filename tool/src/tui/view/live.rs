//! The "Live" tab: a scrolling frame list with a column header and an
//! expandable two-column detail inspector for the selected frame.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};

use crate::tui::App;
use crate::tui::decode::FrameKind;
use crate::tui::model::FrameEntry;
use crate::tui::view::theme::{
    self, ACCENT, ERR, EXT, GOLD, J1939, NMEA, STD, TEXT, accent, bold, dim, fg, meter_split,
    panel, text,
};

// Column widths shared by the header and every data row so they line up.
const W_TIME: usize = 8;
const W_IFACE: usize = 6;
const W_ID: usize = 8;
const W_DLC: usize = 3;
const W_DATA: usize = 23; // 8 bytes * 2 hex + 7 spaces
/// Separator drawn between the DATA and DECODE columns.
const COL_SEP: &str = "  │ ";

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let detail_h = if app.detail_expanded {
        (area.height / 3).clamp(8, 18)
    } else {
        0
    };
    let constraints = if app.detail_expanded {
        vec![Constraint::Min(0), Constraint::Length(detail_h)]
    } else {
        vec![Constraint::Min(0)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let indices = app.filtered_indices();
    let count = indices.len();
    let sel = if app.follow_tail {
        count.saturating_sub(1)
    } else {
        app.selected.min(count.saturating_sub(1))
    };
    app.selected = sel;

    let items: Vec<ListItem> = indices
        .iter()
        .map(|&i| ListItem::new(line_for(&app.frames[i])))
        .collect();

    // Header + activity meter in the list title.
    let (act_filled, _act_empty) = meter_split(app.stats.fps() as u64, 400, 16);
    let title = format!(
        "Live  ·  {} shown / {} buffered  {}",
        count,
        app.frames.len(),
        if app.paused { "⏸ PAUSED" } else { "" },
    );

    // Split the list panel interior into a header line and the list body.
    let list_block = panel(&title);
    let list_inner = list_block.inner(chunks[0]);
    frame.render_widget(list_block, chunks[0]);

    let header_area = Rect {
        height: 1,
        ..list_inner
    };
    let body_area = Rect {
        y: list_inner.y + 1,
        height: list_inner.height.saturating_sub(1),
        ..list_inner
    };
    draw_header(frame, header_area, &act_filled);

    let mut state = ListState::default();
    if count > 0 {
        state.select(Some(sel));
    }
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme::PANEL)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, body_area, &mut state);

    if app.detail_expanded {
        let entry = indices.get(sel).and_then(|&i| app.frames.get(i));
        render_detail(frame, chunks[1], entry, app);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, activity: &str) {
    let line = Line::from(vec![
        Span::styled(format!(" {:>W_TIME$}  ", "time"), dim()),
        Span::styled(format!("{:<W_IFACE$}  ", "iface"), dim()),
        Span::styled(format!("{:<W_ID$}  ", "CAN-ID"), dim()),
        Span::styled(format!("{:<W_DLC$}  ", "DLC"), dim()),
        Span::styled(format!("{:<W_DATA$}", "DATA"), dim()),
        Span::styled(COL_SEP, dim()),
        Span::styled("DECODE", bold(ACCENT)),
        Span::styled(format!("   {activity}"), fg(ACCENT)),
    ]);
    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(theme::PANEL)),
        area,
    );
}

/// Build the single-line summary for a frame.
fn line_for(e: &FrameEntry) -> Line<'static> {
    let rel = e.rel_ms as f64 / 1000.0;
    let id_color = if e.err {
        ERR
    } else if e.extended {
        EXT
    } else {
        STD
    };
    // CAN-ID is always rendered in a fixed W_ID-wide field so standard and
    // extended rows (and the header) stay aligned.
    let id_str = if e.extended {
        format!("{:08X}", e.raw_id)
    } else {
        format!("{:03X}", e.raw_id)
    };

    let mut spans = vec![
        Span::styled(format!(" {:>W_TIME$.3}  ", rel), dim()),
        Span::styled(format!("{:<W_IFACE$}  ", e.iface), fg(TEXT)),
        Span::styled(format!("{:>W_ID$}  ", id_str), bold(id_color)),
        Span::styled(format!("{:<W_DLC$}  ", e.dlc), dim()),
    ];

    let n = (e.dlc as usize).min(8);
    if e.rtr {
        spans.push(Span::styled(format!("{:<W_DATA$}", "RTR"), bold(GOLD)));
    } else {
        let mut hex = String::with_capacity(n * 3 + 2);
        for (i, b) in e.data[..n].iter().enumerate() {
            if i > 0 {
                hex.push(' ');
            }
            hex.push_str(&format!("{b:02X}"));
        }
        spans.push(Span::styled(format!("{hex:<W_DATA$}"), fg(TEXT)));
    }

    // Separator column, then the decode annotation.
    spans.push(Span::styled(COL_SEP, dim()));
    match e.decoded.kind {
        FrameKind::Raw => {
            if e.err {
                spans.push(Span::styled("error frame", bold(ERR)));
            } else {
                spans.push(Span::styled("standard CAN", dim()));
            }
        }
        FrameKind::J1939 => spans.push(Span::styled(decode_str(e), bold(J1939))),
        FrameKind::Nmea2000 => spans.push(Span::styled(decode_str(e), bold(NMEA))),
    }

    Line::from(spans)
}

fn decode_str(e: &FrameEntry) -> String {
    let name = e.decoded.name.unwrap_or("unknown");
    let pgn = e.decoded.pgn.unwrap_or(0);
    let src = e.decoded.source.unwrap_or(0);
    let dst = e.decoded.destination.unwrap_or(0);
    let prio = e.decoded.priority.unwrap_or(0);
    format!("pgn={pgn:05X} p{prio} src={src:02X} dst={dst:02X} [{name}]")
}

/// Two-column detail inspector.
fn render_detail(frame: &mut Frame, area: Rect, entry: Option<&FrameEntry>, app: &App) {
    let block = panel("Detail");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(e) = entry else {
        let hint = if app.frames.is_empty() {
            "waiting for frames…"
        } else {
            "no frame matches the active filter"
        };
        frame.render_widget(Paragraph::new(hint).style(dim()), inner);
        return;
    };

    // Left: addressing + decode fields. Right: hex/ASCII dump.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(40), Constraint::Min(0)])
        .split(inner);

    draw_addressing(frame, cols[0], e);
    draw_dump(frame, cols[1], e);
}

fn draw_addressing(frame: &mut Frame, area: Rect, e: &FrameEntry) {
    let rel = e.rel_ms as f64 / 1000.0;
    let id_color = if e.err {
        ERR
    } else if e.extended {
        EXT
    } else {
        STD
    };

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled(format!("#{}  @{rel:.3}s  ", e.seq), bold(TEXT)),
            Span::styled(e.iface.clone(), accent()),
            Span::styled(if e.extended { "  EXT" } else { "  STD" }, bold(id_color)),
        ]),
        Line::from(""),
    ];

    if let Some(pgn) = e.decoded.pgn {
        lines.push(field_line("raw", format!("{:08X}", e.raw_id)));
        lines.push(field_line_color("pgn", format!("{pgn:05X}"), ACCENT));
        lines.push(field_line(
            "priority",
            e.decoded.priority.unwrap_or(0).to_string(),
        ));
        lines.push(field_line_color(
            "source",
            format!("{:02X}", e.decoded.source.unwrap_or(0)),
            EXT,
        ));
        lines.push(field_line(
            "dest",
            format!("{:02X}", e.decoded.destination.unwrap_or(0)),
        ));
        if let Some(name) = e.decoded.name {
            let col = if e.decoded.kind == FrameKind::Nmea2000 {
                NMEA
            } else {
                J1939
            };
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("  name  ", dim()),
                Span::styled(name, bold(col)),
            ]));
        }
    } else if e.err {
        lines.push(Line::from(Span::styled("  ERROR FRAME", bold(ERR))));
    } else {
        lines.push(Line::from(Span::styled("  standard CAN frame", dim())));
    }

    // Decoded fields.
    if !e.decoded.fields.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  decoded fields", accent())));
        for (k, v) in &e.decoded.fields {
            lines.push(Line::from(vec![
                Span::styled(format!("    {:<12}", k), dim()),
                Span::styled(v.clone(), bold(GOLD)),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_dump(frame: &mut Frame, area: Rect, e: &FrameEntry) {
    let n = (e.dlc as usize).min(8);
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(Span::styled("data", accent())));
    lines.push(Line::from(""));

    if e.rtr {
        lines.push(Line::from(Span::styled(
            "  remote request (RTR)",
            bold(GOLD),
        )));
    } else {
        // Hex with byte offsets, plus an ASCII gutter.
        for row in 0..n.div_ceil(8).max(1) {
            let start = row * 8;
            let mut hex = String::new();
            let mut ascii = String::new();
            for off in 0..8 {
                let idx = start + off;
                if idx < n {
                    hex.push_str(&format!("{:02X} ", e.data[idx]));
                    ascii.push(if (0x20..=0x7E).contains(&e.data[idx]) {
                        e.data[idx] as char
                    } else {
                        '·'
                    });
                } else {
                    hex.push_str("   ");
                    ascii.push(' ');
                }
            }
            lines.push(Line::from(vec![
                Span::styled(format!("  {:02X}  ", start), dim()),
                Span::styled(hex, fg(TEXT)),
                Span::styled(format!("  ▏{ascii}"), dim()),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn field_line(k: &str, v: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<9}", k), dim()),
        Span::styled(v, fg(TEXT)),
    ])
}
fn field_line_color(k: &str, v: String, col: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<9}", k), dim()),
        Span::styled(v, bold(col)),
    ])
}

// silence unused import warnings for palette entries kept for reachability
#[allow(dead_code)]
fn _reach() {
    let _ = (text(), panel(""));
}
