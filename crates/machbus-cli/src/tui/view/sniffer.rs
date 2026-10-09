//! The "Sniffer" tab: a stationary grid keyed by CAN ID. Each row shows the
//! current payload bytes; a byte that changed within the hold window is
//! highlighted, so changes "flash" and stable bytes stay quiet.

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};

use crate::tui::App;
use crate::tui::model::{SNIFF_HOLD_MS, SniffRow};
use crate::tui::view::theme::{self, ACCENT, EXT, STD, TEXT, bold, dim, fg, panel};

const HOLD: Duration = Duration::from_millis(SNIFF_HOLD_MS);

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        "Sniffer  ·  {} IDs  ·  {} ms hold",
        app.sniff.rows.len(),
        SNIFF_HOLD_MS,
    );
    let block = panel(&title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Header row: byte offsets (low → high, little-endian signal order).
    let hdr = vec![
        Span::styled(format!(" {:<10}  ", "CAN-ID"), dim()),
        Span::styled("B0   B1   B2   B3   B4   B5   B6   B7", dim()),
        Span::styled(format!("   {:>8}", "updates"), dim()),
    ];

    let mut rows: Vec<&SniffRow> = app.sniff.rows.values().collect();
    rows.sort_by(|a, b| a.can_id.cmp(&b.can_id));

    let now = Instant::now();
    let mut items: Vec<ListItem> = Vec::with_capacity(rows.len() + 1);
    items.push(ListItem::new(Line::from(hdr)));

    for row in &rows {
        let id_color = if row.extended { EXT } else { STD };
        let id_str = if row.extended {
            format!("{:08X}", row.can_id)
        } else {
            format!("{:03X}", row.can_id)
        };
        let mut spans = vec![Span::styled(format!(" {:<10}  ", id_str), bold(id_color))];

        for i in 0..8 {
            let cell = if (i as u8) < row.dlc {
                let hot = row.changed_at[i].is_some_and(|t| now.duration_since(t) < HOLD);
                let style = if hot { bold(ACCENT) } else { fg(TEXT) };
                Span::styled(format!("{:02X}  ", row.data[i]), style)
            } else {
                Span::styled("··  ", dim())
            };
            spans.push(cell);
        }
        spans.push(Span::styled(format!("  {:>8}", row.count), dim()));
        items.push(ListItem::new(Line::from(spans)));
    }

    if rows.is_empty() {
        items.push(ListItem::new(Span::styled("  no frames yet", dim())));
    }

    frame.render_widget(
        List::new(items).highlight_style(Style::default().bg(theme::PANEL)),
        inner,
    );
}
