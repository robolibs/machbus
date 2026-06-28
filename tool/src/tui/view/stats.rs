//! The "Stats" tab: a dashboard of gauges (buffer / bus load), a frame-rate
//! sparkline, and metered top-PGN / per-interface breakdowns.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, List, ListItem, Paragraph, Sparkline, Wrap};

use crate::tui::App;
use crate::tui::decode::FrameKind;
use crate::tui::view::theme::{
    self, ACCENT, EXT, GOLD, J1939, NMEA, STD, TEXT, bold, dim, fg, meter_split, panel, panel_plain,
};

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(0)])
        .split(area);

    draw_counters(frame, app, cols[0]);
    draw_dashboard(frame, app, cols[1]);
}

fn draw_counters(frame: &mut Frame, app: &App, area: Rect) {
    let block = panel("Counters");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let elapsed = app.stats.elapsed_secs();
    let rate = if elapsed > 0.0 {
        app.stats.total as f64 / elapsed
    } else {
        0.0
    };

    let mut lines = vec![
        kv("Total", app.stats.total.to_string(), TEXT),
        kv(
            "In buffer",
            format!("{} / {}", app.frames.len(), app.buffer_cap),
            ACCENT,
        ),
        kv("Distinct PGNs", app.stats.per_pgn.len().to_string(), TEXT),
        Line::from(""),
        kv("Extended", app.stats.ext.to_string(), EXT),
        kv("Standard", app.stats.std.to_string(), STD),
        kv("RTR", app.stats.rtr.to_string(), TEXT),
        kv("Error", app.stats.err.to_string(), theme::ERR),
        kv("Data bytes", app.stats.bytes.to_string(), TEXT),
        Line::from(""),
        kv("Elapsed", format!("{elapsed:.1} s"), TEXT),
        kv("Rate", format!("{rate:.1} f/s"), GOLD),
        kv(
            "Filter",
            format!("{} constraint(s)", app.filter.active_count()),
            GOLD,
        ),
    ];

    // Per-interface as a small list under the counters.
    if !app.stats.per_iface.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("interfaces", bold(ACCENT))));
        let mut ifaces: Vec<(&String, &u64)> = app.stats.per_iface.iter().collect();
        ifaces.sort_by(|a, b| b.1.cmp(a.1));
        for (name, n) in ifaces.iter().take(4) {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<8}", name), fg(Color::White)),
                Span::styled(format!("{n:>8}"), fg(ACCENT)),
            ]));
        }
    }

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().fg(TEXT)),
        inner,
    );
}

fn draw_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // gauges row
            Constraint::Length(5), // sparkline
            Constraint::Min(0),    // top PGNs
        ])
        .split(area);

    // ── gauges ──
    let g = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);
    let buf_pct =
        ((app.frames.len() as f64 / app.buffer_cap as f64) * 100.0).clamp(0.0, 100.0) as u16;
    let load_pct = bus_load_percent(app);
    frame.render_widget(
        Gauge::default()
            .block(panel_plain().title(format!("Buffer  {:>3}%", buf_pct)))
            .gauge_style(Style::default().fg(ACCENT))
            .percent(buf_pct),
        g[0],
    );
    let load_col = if load_pct > 75 {
        theme::ERR
    } else if load_pct > 40 {
        GOLD
    } else {
        EXT
    };
    frame.render_widget(
        Gauge::default()
            .block(panel_plain().title(format!("Est. bus load  {:>3}%", load_pct)))
            .gauge_style(Style::default().fg(load_col))
            .percent(load_pct),
        g[1],
    );

    // ── sparkline ──
    let data: Vec<u64> = app.rate_history.iter().copied().collect();
    let max = data.iter().copied().max().unwrap_or(1).max(1);
    frame.render_widget(
        Sparkline::default()
            .block(panel("Frame-rate history"))
            .data(&data)
            .max(max)
            .style(fg(ACCENT)),
        chunks[1],
    );

    // ── top PGNs with meters ──
    let block = panel("Top PGNs by count");
    let inner = block.inner(chunks[2]);
    frame.render_widget(block, chunks[2]);

    let mut top: Vec<&crate::tui::model::PgnStat> = app.stats.per_pgn.values().collect();
    top.sort_by(|a, b| b.count.cmp(&a.count));
    let cap = inner.height as usize;
    let max_count = top.first().map(|s| s.count).unwrap_or(1).max(1);

    let mut items: Vec<ListItem> = Vec::new();
    for s in top.iter().take(cap) {
        let col = if s.kind == FrameKind::Nmea2000 {
            NMEA
        } else {
            J1939
        };
        let (filled, empty) = meter_split(s.count, max_count, 14);
        let name = s.name.unwrap_or("unknown");
        let line = Line::from(vec![
            Span::styled(format!("{:05X} ", s.pgn), bold(col)),
            Span::styled(format!("{:<22}", name), fg(TEXT)),
            Span::styled(filled, fg(col)),
            Span::styled(empty, dim()),
            Span::styled(format!(" {:>6}", s.count), fg(ACCENT)),
            Span::styled(format!("  src {:02X}", s.last_src), dim()),
        ]);
        items.push(ListItem::new(line));
    }
    if items.is_empty() {
        items.push(ListItem::new(Span::styled("  no traffic yet", dim())));
    }
    frame.render_widget(List::new(items), inner);

    let _ = (Wrap { trim: false }, symbols::line::HORIZONTAL);
}

/// Rough bus-load estimate: classical-CAN ≈ 111 bits/frame at 250 kbit/s.
fn bus_load_percent(app: &App) -> u16 {
    let elapsed = app.stats.elapsed_secs();
    if elapsed <= 0.0 {
        return 0;
    }
    let bits_per_frame = 111.0;
    let bitrate = 250_000.0;
    let load = (app.stats.total as f64 * bits_per_frame) / (elapsed * bitrate) * 100.0;
    load.round() as u16
}

fn kv(k: &str, v: String, col: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<13}", k), dim()),
        Span::styled(v, bold(col)),
    ])
}
