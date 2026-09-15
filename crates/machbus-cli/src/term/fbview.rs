//! Paint a machbus VT [`Framebuffer`] into a ratatui [`Buffer`] using
//! **braille** characters.
//!
//! Each terminal cell maps to a 2 × 4 pixel grid (8 sub-pixels).  For every
//! cell we sample the 8 sub-pixels, compute the cell's average colour as the
//! foreground, then light a dot where the sub-pixel's luminance is at or
//! above the cell average and leave it dark otherwise.  This is the same
//! technique used by `chafa` / `timg` for braille image rendering: uniform
//! areas become solid blocks of colour, while edges and text show sub-cell
//! detail via lit/dark dots — dramatically sharper than half-blocks.

use machbus::isobus::vt::render::framebuffer::Framebuffer;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

type Rgb = (u8, u8, u8);

/// Unicode braille bit-positions for a 4-row × 2-col sub-cell grid.
///
/// ```text
/// 0x01 0x08    (row 0)
/// 0x02 0x10    (row 1)
/// 0x04 0x20    (row 2)
/// 0x40 0x80    (row 3)
/// ```
const BRAILLE_BITS: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

/// Paint `fb` into `buf` over `area`, fitting, centring, and using braille
/// for maximum sub-cell resolution.
pub fn paint(buf: &mut Buffer, area: Rect, fb: &Framebuffer) {
    let fw = f64::from(fb.width());
    let fh = f64::from(fb.height());
    if fw == 0.0 || fh == 0.0 || area.width == 0 || area.height == 0 {
        return;
    }

    // Braille sub-cell resolution: 2 wide × 4 tall per terminal cell.
    let disp_w = f64::from(area.width) * 2.0;
    let disp_h = f64::from(area.height) * 4.0;
    let scale = (disp_w / fw).min(disp_h / fh);
    let geo = Geo {
        ox: (disp_w - fw * scale) / 2.0,
        oy: (disp_h - fh * scale) / 2.0,
        inv: 1.0 / scale,
        fw,
        fh,
    };

    for cy in 0..area.height {
        for cx in 0..area.width {
            let base_x = f64::from(cx) * 2.0;
            let base_y = f64::from(cy) * 4.0;

            // --- pass 1: sample 8 sub-pixels ---
            let mut samples: [Option<Rgb>; 8] = [None; 8];
            let mut sr = 0u32;
            let mut sg = 0u32;
            let mut sb = 0u32;
            let mut n = 0u32;

            for row in 0..4u32 {
                for col in 0..2u32 {
                    let dx = base_x + col as f64;
                    let dy = base_y + row as f64;
                    if let Some(c) = geo.sample(fb, dx, dy) {
                        let idx = (row * 2 + col) as usize;
                        samples[idx] = Some(c);
                        sr += u32::from(c.0);
                        sg += u32::from(c.1);
                        sb += u32::from(c.2);
                        n += 1;
                    }
                }
            }

            let cell = &mut buf[(area.x + cx, area.y + cy)];
            if n == 0 {
                cell.set_symbol(" ");
                cell.set_style(Style::default());
                continue;
            }

            // --- pass 2: luminance-threshold the dots ---
            let avg_r = sr / n;
            let avg_g = sg / n;
            let avg_b = sb / n;
            let avg_lum = (avg_r + avg_g + avg_b) / 3;

            let mut bits: u8 = 0;
            for (row_idx, row_bits) in BRAILLE_BITS.iter().enumerate() {
                for (col_idx, &bit) in row_bits.iter().enumerate() {
                    let idx = row_idx * 2 + col_idx;
                    if let Some((r, g, b)) = samples[idx] {
                        let lum = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
                        if lum >= avg_lum {
                            bits |= bit;
                        }
                    }
                }
            }
            // Guarantee at least one lit dot so the cell is never invisible.
            if bits == 0 {
                bits = BRAILLE_BITS[0][0];
            }

            // Encode braille char (U+2800 + bits) into a stack buffer.
            let mut char_buf = [0u8; 4];
            let code = 0x2800u32 + u32::from(bits);
            let symbol = char::from_u32(code)
                .unwrap_or('\u{2800}')
                .encode_utf8(&mut char_buf);

            cell.set_symbol(symbol);
            cell.set_style(Style::default().fg(Color::Rgb(avg_r as u8, avg_g as u8, avg_b as u8)));
        }
    }
}

/// Map a clicked terminal cell (relative to `area`) to the corresponding VT
/// framebuffer pixel.  Returns `None` when the cell falls outside the image.
#[must_use]
pub fn screen_to_pixel(
    area: Rect,
    fb: &Framebuffer,
    cell_x: u16,
    cell_y: u16,
) -> Option<(i32, i32)> {
    let fw = f64::from(fb.width());
    let fh = f64::from(fb.height());
    if fw == 0.0 || fh == 0.0 || area.width == 0 || area.height == 0 {
        return None;
    }
    let disp_w = f64::from(area.width) * 2.0;
    let disp_h = f64::from(area.height) * 4.0;
    let scale = (disp_w / fw).min(disp_h / fh);
    let ox = (disp_w - fw * scale) / 2.0;
    let oy = (disp_h - fh * scale) / 2.0;
    // Centre of the clicked cell in display (braille) coordinates.
    let dx = f64::from(cell_x) * 2.0 + 1.0;
    let dy = f64::from(cell_y) * 4.0 + 2.0;
    let fx = ((dx - ox) / scale) as i32;
    let fy = ((dy - oy) / scale) as i32;
    if fx >= 0 && fy >= 0 && (fx as u16) < fb.width() && (fy as u16) < fb.height() {
        Some((fx, fy))
    } else {
        None
    }
}

/// Geometry mapping display (braille sub-cell) coordinates → framebuffer
/// regions.
struct Geo {
    ox: f64,
    oy: f64,
    inv: f64,
    fw: f64,
    fh: f64,
}

impl Geo {
    /// Average the framebuffer pixels covered by display pixel `(dx, dy)`.
    fn sample(&self, fb: &Framebuffer, dx: f64, dy: f64) -> Option<Rgb> {
        let fx0 = (dx - self.ox) * self.inv;
        let fy0 = (dy - self.oy) * self.inv;
        let fx1 = fx0 + self.inv;
        let fy1 = fy0 + self.inv;
        if fx1 <= 0.0 || fy1 <= 0.0 || fx0 >= self.fw || fy0 >= self.fh {
            return None;
        }

        let x0 = fx0.floor().max(0.0) as u16;
        let y0 = fy0.floor().max(0.0) as u16;
        let x1 = (fx1.ceil().min(self.fw) as u16).max(x0.saturating_add(1));
        let y1 = (fy1.ceil().min(self.fh) as u16).max(y0.saturating_add(1));

        let (mut sr, mut sg, mut sb, mut n) = (0u32, 0u32, 0u32, 0u32);
        for yy in y0..y1 {
            for xx in x0..x1 {
                if let Some(c) = fb.pixel(xx, yy) {
                    sr += u32::from(c.r);
                    sg += u32::from(c.g);
                    sb += u32::from(c.b);
                    n += 1;
                }
            }
        }
        if n == 0 {
            None
        } else {
            Some(((sr / n) as u8, (sg / n) as u8, (sb / n) as u8))
        }
    }
}
