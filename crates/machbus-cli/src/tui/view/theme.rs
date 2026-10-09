//! Visual theme for `mechdump live`: a cohesive truecolor palette plus
//! reusable styled-block and meter helpers. Centralizing the look here keeps
//! every tab consistent and easy to retune.

use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders};

// ── palette ─────────────────────────────────────────────────────────────
//
// This theme uses ONLY the terminal's own ANSI palette — the named
// `Color::*` variants (which map to your terminal's color slots 0–15) and
// `Color::Reset` for the background. Nothing is hard-coded to RGB or a
// fixed 256-cube index, so the UI inherits whatever theme/opacity your
// terminal is configured with (transparent backgrounds, custom reds,
// solarized, etc.). To ship a fixed look instead, swap these `const`s for
// `Color::Rgb(..)` or `Color::Indexed(n)`.
//
/// App background = the terminal's default (respects theme + transparency,
/// never a hard-coded "pitch black").
pub const BG: Color = Color::Reset;
/// Selection / header row fill (ANSI 8 — bright black / dark gray).
pub const PANEL: Color = Color::DarkGray;
/// Panel borders (ANSI 8).
pub const BORDER: Color = Color::DarkGray;
/// Primary text (ANSI 15 — bright white).
pub const TEXT: Color = Color::White;
/// Secondary / hint text (ANSI 7 — gray).
pub const DIM: Color = Color::Gray;
/// Brand accent (ANSI 1 — red, from the terminal palette).
pub const ACCENT: Color = Color::Red;
/// Warning / highlight (ANSI 3 — yellow).
pub const GOLD: Color = Color::Yellow;
/// Extended (29-bit) CAN IDs (ANSI 2 — green).
pub const EXT: Color = Color::Green;
/// Standard (11-bit) CAN IDs (ANSI 4 — blue).
pub const STD: Color = Color::Blue;
/// Error frames (ANSI 9 — bright red).
pub const ERR: Color = Color::LightRed;
/// J1939 / ISOBUS decode (green).
pub const J1939: Color = Color::Green;
/// NMEA 2000 decode (ANSI 5 — magenta).
pub const NMEA: Color = Color::Magenta;
/// Good / live indicator (green).
pub const OK: Color = Color::Green;

// ── style helpers ───────────────────────────────────────────────────────
#[must_use]
pub fn fg(c: Color) -> Style {
    Style::default().fg(c)
}
#[must_use]
pub fn bold(c: Color) -> Style {
    Style::default().fg(c).add_modifier(Modifier::BOLD)
}
#[must_use]
pub fn dim() -> Style {
    fg(DIM)
}
#[must_use]
pub fn text() -> Style {
    fg(TEXT)
}
#[must_use]
pub fn accent() -> Style {
    bold(ACCENT)
}

// ── blocks ──────────────────────────────────────────────────────────────
/// A rounded, titled panel.
#[must_use]
pub fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(fg(BORDER))
        .title(Line::from(format!(" {title} ")).style(bold(ACCENT)))
}

/// A rounded, untitled panel.
#[must_use]
pub fn panel_plain() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(fg(BORDER))
}

// ── meters ──────────────────────────────────────────────────────────────
const EIGHTH: [char; 7] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// Render `value/max` as a horizontal block-meter of `width` cells, using
/// eighths for sub-cell resolution. The filled portion is meant to be styled
/// by the caller; the empty cells use `░`.
#[must_use]
pub fn meter(value: u64, max: u64, width: usize) -> String {
    let max = max.max(1);
    let ratio = (value as f64 / max as f64).clamp(0.0, 1.0);
    let eighths = (ratio * width as f64 * 8.0).round() as usize;
    let full = eighths / 8;
    let rem = eighths % 8;
    let mut s = String::with_capacity(width);
    for _ in 0..full.min(width) {
        s.push('█');
    }
    if rem > 0 && full < width {
        s.push(EIGHTH[rem - 1]);
    }
    while s.chars().count() < width {
        s.push('░');
    }
    s
}

/// Split a meter string into `(filled, empty)` halves so the caller can
/// color them differently.
#[must_use]
pub fn meter_split(value: u64, max: u64, width: usize) -> (String, String) {
    let full = meter(value, max, width);
    let cut = full.find('░').unwrap_or(full.len());
    let filled = full[..cut].to_string();
    let empty = full[cut..].to_string();
    (filled, empty)
}
