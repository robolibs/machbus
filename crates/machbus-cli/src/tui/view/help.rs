//! The "Help" tab: keybindings reference.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::tui::view::theme::{ACCENT, TEXT, bold, dim, panel};

pub fn render(frame: &mut Frame, _app: &mut crate::tui::App, area: Rect) {
    let block = panel("Help / Keybindings");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(help_lines()), inner);
}

fn help_lines() -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();

    section(&mut out, "Global");
    row(&mut out, "q / Ctrl+C", "quit");
    row(&mut out, "Tab / ]", "next tab");
    row(&mut out, "Shift+Tab / [", "previous tab");
    row(&mut out, "1..8", "jump to tab");
    row(&mut out, "p / Space", "pause / resume capture");
    row(&mut out, "c", "clear buffer + stats");
    row(&mut out, "f", "open the Filter tab");
    row(&mut out, "/", "quick data-text filter");
    row(&mut out, "?", "this help");

    section(&mut out, "Tabs");
    row(&mut out, "1 Live", "scrolling frame log + decode detail");
    row(&mut out, "2 Sniffer", "per-ID grid, changed bytes flash");
    row(&mut out, "3 PGN", "J1939/ISOBUS PGN aggregate");
    row(&mut out, "4 NMEA 2000", "NMEA PGN aggregate + decode");
    row(&mut out, "5 Nodes", "J1939 address-claim table");
    row(&mut out, "6 Stats", "counters, gauges, rate sparkline");
    row(&mut out, "7 Filter", "structured capture filter");

    section(&mut out, "Live");
    row(&mut out, "j/k ↑↓", "move selection (stops tail-follow)");
    row(&mut out, "PgDn / PgUp", "move by 10");
    row(&mut out, "g / G", "jump to top / bottom (G resumes follow)");
    row(&mut out, "d / Enter", "toggle the detail pane");
    row(&mut out, "End", "follow the newest frame");

    section(&mut out, "PGN / NMEA");
    row(&mut out, "j/k ↑↓", "move selection");
    row(&mut out, "Enter", "filter Live by the selected PGN");

    section(&mut out, "Filter");
    row(&mut out, "j/k / Tab", "next / previous field");
    row(&mut out, "Enter", "edit the highlighted field");
    row(&mut out, "x", "clear the highlighted field");
    row(&mut out, "e / s", "toggle extended-only / standard-only");
    row(&mut out, "J / N", "J1939-only / NMEA-only kind filter");
    row(&mut out, "A", "clear all kind filters");
    row(&mut out, "C", "clear the entire filter");

    section(&mut out, "Editing a field");
    row(&mut out, "Enter", "save the value");
    row(&mut out, "Esc", "cancel");
    row(&mut out, "← → Home End", "move the cursor");
    row(&mut out, "Backspace", "delete before cursor");

    out
}

fn section(out: &mut Vec<Line<'static>>, title: &str) {
    out.push(Line::from(""));
    out.push(Line::from(Span::styled(title.to_string(), bold(ACCENT))));
}

fn row(out: &mut Vec<Line<'static>>, k: &str, d: &str) {
    out.push(Line::from(vec![
        Span::styled(format!("  {:<14}", k), bold(TEXT)),
        Span::styled(d.to_string(), dim()),
    ]));
}
