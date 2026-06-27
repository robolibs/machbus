//! Grid-based site-specific application (ISO 11783-10 Grid type 1).
//!
//! Besides polygon treatment zones ([`super::geo`]), a task may carry a regular
//! grid: a rectangular array of cells, each holding a treatment-zone code, laid
//! out row-major from a south-west origin with fixed cell sizes in degrees. This
//! is the repo-owned in-memory grid plus position→cell lookup; decoding the
//! binary grid file into one is a later slice.

use alloc::vec::Vec;

use crate::geo::Wgs;

/// A rectangular treatment-zone grid (Grid type 1: one zone code per cell).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreatmentZoneGrid {
    /// South-west corner: minimum latitude/longitude of the grid.
    pub origin: Wgs,
    /// Cell height, degrees latitude (north spacing).
    pub cell_lat_deg: f64,
    /// Cell width, degrees longitude (east spacing).
    pub cell_lon_deg: f64,
    /// Number of latitude cells (rows, south→north).
    pub rows: u32,
    /// Number of longitude cells (columns, west→east).
    pub cols: u32,
    /// Row-major cells (`row * cols + col`), one treatment-zone code each.
    pub cells: Vec<u8>,
}

impl TreatmentZoneGrid {
    /// `true` if the cell buffer matches `rows * cols` and the cell sizes are
    /// positive.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.cell_lat_deg > 0.0
            && self.cell_lon_deg > 0.0
            && self.cells.len() == (self.rows as usize) * (self.cols as usize)
    }

    /// The `(row, col)` a position falls in, or `None` if outside the grid.
    #[must_use]
    pub fn cell_rc(&self, pos: Wgs) -> Option<(u32, u32)> {
        if self.cell_lat_deg <= 0.0 || self.cell_lon_deg <= 0.0 {
            return None;
        }
        let dlat = pos.latitude - self.origin.latitude;
        let dlon = pos.longitude - self.origin.longitude;
        if dlat < 0.0 || dlon < 0.0 {
            return None;
        }
        let row = (dlat / self.cell_lat_deg) as u32;
        let col = (dlon / self.cell_lon_deg) as u32;
        if row >= self.rows || col >= self.cols {
            return None;
        }
        Some((row, col))
    }

    /// The treatment-zone code at a position, or `None` if outside the grid.
    #[must_use]
    pub fn zone_at(&self, pos: Wgs) -> Option<u8> {
        let (row, col) = self.cell_rc(pos)?;
        self.cells
            .get((row as usize) * (self.cols as usize) + col as usize)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_lookup_maps_position_to_zone_code() {
        // 2 rows × 3 cols, 0.001° cells, origin at (52.000, 5.000).
        let grid = TreatmentZoneGrid {
            origin: Wgs::new(52.000, 5.000, 0.0),
            cell_lat_deg: 0.001,
            cell_lon_deg: 0.001,
            rows: 2,
            cols: 3,
            cells: vec![10, 11, 12, 20, 21, 22],
        };
        assert!(grid.is_valid());

        // Origin cell (row 0, col 0).
        assert_eq!(grid.zone_at(Wgs::new(52.0000, 5.0000, 0.0)), Some(10));
        // Row 0, col 2.
        assert_eq!(grid.zone_at(Wgs::new(52.0005, 5.0025, 0.0)), Some(12));
        // Row 1, col 1.
        assert_eq!(grid.zone_at(Wgs::new(52.0015, 5.0015, 0.0)), Some(21));
        // Outside (south/west of origin, and north/east past the grid).
        assert_eq!(grid.zone_at(Wgs::new(51.9990, 5.0000, 0.0)), None);
        assert_eq!(grid.zone_at(Wgs::new(52.0030, 5.0000, 0.0)), None);

        // An inconsistent cell buffer is invalid.
        let bad = TreatmentZoneGrid {
            cells: vec![1, 2],
            ..grid
        };
        assert!(!bad.is_valid());
    }
}
