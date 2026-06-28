//! The "NMEA 2000" tab: aggregate of NMEA 2000 PGNs (reuses the PGN table).

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::tui::App;
use crate::tui::decode::FrameKind;
use crate::tui::view::pgn;

pub fn render(frame: &mut Frame, app: &mut App, area: Rect) {
    // Build the NMEA row set first so we can record it for key handling.
    let rows = pgn::build_rows(app, FrameKind::Nmea2000);
    app.nmea_rows = rows.clone();
    let title = format!(
        "NMEA 2000 PGNs  ·  {} distinct  ·  Enter filters this PGN in Live",
        rows.len(),
    );
    pgn::draw_nmea_table(frame, app, area, &rows, &title);
}
