//! The zone grid: what the player painted, one byte per square cell (ADR 0014).

/// Side of one zone cell, metres.
pub const ZONE_CELL: f32 = 8.0;
/// Land past the starting road network that can still be zoned, metres.
pub const ZONE_MARGIN: f32 = 480.0;

/// A zone, including its density. The discriminant is the stored cell byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Zone {
    None = 0,
    ResidentialLow = 1,
    ResidentialHigh = 2,
    CommercialLow = 3,
    CommercialHigh = 4,
    IndustrialLow = 5,
    IndustrialHigh = 6,
}

impl Zone {
    pub fn from_u8(v: u8) -> Option<Zone> {
        Some(match v {
            0 => Zone::None,
            1 => Zone::ResidentialLow,
            2 => Zone::ResidentialHigh,
            3 => Zone::CommercialLow,
            4 => Zone::CommercialHigh,
            5 => Zone::IndustrialLow,
            6 => Zone::IndustrialHigh,
            _ => return None,
        })
    }
}

/// A fixed, row-major grid of zone cells. Cell `(c, r)` covers
/// `[x0 + c * ZONE_CELL, x0 + (c + 1) * ZONE_CELL)` and likewise in y.
pub struct ZoneGrid {
    pub x0: f32,
    pub y0: f32,
    pub cols: u32,
    pub rows: u32,
    pub cells: Vec<u8>,
    /// Bumped on every change so the frontend knows when to redraw.
    pub version: u32,
}

impl ZoneGrid {
    /// A grid covering `[min_x, min_y, max_x, max_y]` plus `ZONE_MARGIN`, aligned to whole cells.
    /// Empty bounds (min > max, as for a network with no nodes) cover the margin around the origin.
    pub fn covering(bounds: [f32; 4]) -> Self {
        let [min_x, min_y, max_x, max_y] = if bounds[0] <= bounds[2] && bounds[1] <= bounds[3] {
            bounds
        } else {
            [0.0; 4]
        };
        let x0 = ((min_x - ZONE_MARGIN) / ZONE_CELL).floor() * ZONE_CELL;
        let y0 = ((min_y - ZONE_MARGIN) / ZONE_CELL).floor() * ZONE_CELL;
        let cols = ((max_x + ZONE_MARGIN - x0) / ZONE_CELL).ceil().max(1.0) as u32;
        let rows = ((max_y + ZONE_MARGIN - y0) / ZONE_CELL).ceil().max(1.0) as u32;
        Self {
            x0,
            y0,
            cols,
            rows,
            cells: vec![Zone::None as u8; cols as usize * rows as usize],
            version: 0,
        }
    }

    /// The zone of the cell containing `(x, y)`, or `None` outside the grid.
    pub fn zone_at(&self, x: f32, y: f32) -> Option<Zone> {
        let c = ((x - self.x0) / ZONE_CELL).floor();
        let r = ((y - self.y0) / ZONE_CELL).floor();
        if !(c >= 0.0 && r >= 0.0 && c < self.cols as f32 && r < self.rows as f32) {
            return None;
        }
        Zone::from_u8(self.cells[r as usize * self.cols as usize + c as usize])
    }

    /// Sets every cell whose centre lies inside the rectangle with corners `(ax, ay)` and
    /// `(bx, by)`, clipped to the grid. Returns whether any cell changed.
    pub fn paint(&mut self, ax: f32, ay: f32, bx: f32, by: f32, zone: Zone) -> bool {
        if ![ax, ay, bx, by].iter().all(|v| v.is_finite()) {
            return false;
        }
        // Cell c's centre is x0 + (c + 0.5) * ZONE_CELL; keep the ones in [lo, hi].
        let span = |lo: f32, hi: f32, origin: f32, n: u32| {
            let first = ((lo - origin) / ZONE_CELL - 0.5).ceil().max(0.0);
            let last = ((hi - origin) / ZONE_CELL - 0.5)
                .floor()
                .min(n as f32 - 1.0);
            (first as usize, last as i64)
        };
        let (c0, c1) = span(ax.min(bx), ax.max(bx), self.x0, self.cols);
        let (r0, r1) = span(ay.min(by), ay.max(by), self.y0, self.rows);
        let mut changed = false;
        for r in r0 as i64..=r1 {
            let row = r as usize * self.cols as usize;
            for c in c0 as i64..=c1 {
                let cell = &mut self.cells[row + c as usize];
                changed |= *cell != zone as u8;
                *cell = zone as u8;
            }
        }
        if changed {
            self.version = self.version.wrapping_add(1);
        }
        changed
    }

    /// `[origin_x, origin_y, cell_size, cols, rows]`.
    pub fn layout(&self) -> [f32; 5] {
        [
            self.x0,
            self.y0,
            ZONE_CELL,
            self.cols as f32,
            self.rows as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> ZoneGrid {
        // 0..100 plus the margin on each side.
        ZoneGrid::covering([0.0, 0.0, 100.0, 100.0])
    }

    #[test]
    fn covers_bounds_plus_margin_on_whole_cells() {
        let g = grid();
        assert_eq!((g.x0, g.y0), (-ZONE_MARGIN, -ZONE_MARGIN));
        assert_eq!(
            g.cols,
            ((100.0 + 2.0 * ZONE_MARGIN) / ZONE_CELL).ceil() as u32
        );
        assert_eq!(g.cells.len(), (g.cols * g.rows) as usize);
        assert_eq!(g.zone_at(50.0, 50.0), Some(Zone::None));
        assert_eq!(g.zone_at(-ZONE_MARGIN - 1.0, 0.0), None);
        let empty = ZoneGrid::covering([f32::MAX, f32::MAX, f32::MIN, f32::MIN]);
        assert_eq!(empty.cols, (2.0 * ZONE_MARGIN / ZONE_CELL) as u32);
    }

    #[test]
    fn paints_cells_whose_centres_are_inside_either_corner_order() {
        let mut g = grid();
        // Cells with centres 4, 12, 20 (x) and 4 (y) are inside; 28 is not.
        assert!(g.paint(24.0, 6.0, 0.0, 0.0, Zone::CommercialHigh));
        assert_eq!(g.zone_at(1.0, 1.0), Some(Zone::CommercialHigh));
        assert_eq!(g.zone_at(23.0, 7.0), Some(Zone::CommercialHigh));
        assert_eq!(g.zone_at(25.0, 1.0), Some(Zone::None));
        assert_eq!(g.zone_at(1.0, 9.0), Some(Zone::None));
        assert_eq!(g.version, 1);
    }

    #[test]
    fn repainting_the_same_zone_changes_nothing_and_unzone_clears() {
        let mut g = grid();
        assert!(g.paint(0.0, 0.0, 40.0, 40.0, Zone::ResidentialLow));
        assert!(!g.paint(0.0, 0.0, 40.0, 40.0, Zone::ResidentialLow));
        assert_eq!(g.version, 1);
        assert!(g.paint(0.0, 0.0, 40.0, 40.0, Zone::ResidentialHigh));
        assert_eq!(g.zone_at(10.0, 10.0), Some(Zone::ResidentialHigh));
        assert!(g.paint(0.0, 0.0, 40.0, 40.0, Zone::None));
        assert_eq!(g.zone_at(10.0, 10.0), Some(Zone::None));
    }

    #[test]
    fn clips_to_the_grid_and_ignores_bad_input() {
        let mut g = grid();
        assert!(g.paint(-1e6, -1e6, 1e6, 1e6, Zone::IndustrialLow));
        assert!(g.cells.iter().all(|&c| c == Zone::IndustrialLow as u8));
        let mut g = grid();
        assert!(!g.paint(1e6, 1e6, 2e6, 2e6, Zone::IndustrialLow));
        assert!(!g.paint(f32::NAN, 0.0, 10.0, 10.0, Zone::IndustrialLow));
        assert!(!g.paint(0.0, 0.0, f32::INFINITY, 10.0, Zone::IndustrialLow));
        // A rectangle smaller than a cell that misses every centre paints nothing.
        assert!(!g.paint(0.5, 0.5, 1.5, 1.5, Zone::IndustrialLow));
        assert_eq!(g.version, 0);
    }
}
