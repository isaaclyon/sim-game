//! Finding blocks, their cells and the roads near a point.

use super::{Blocks, Cell, ROAD_HALF_WIDTH, Rect, contains_rect, intersects};
use crate::geometry::{Point, dist, lerp, project};
use crate::road::RoadId;
use crate::zone::ZoneGrid;

impl Blocks {
    pub fn contains(&self, i: usize, p: Point) -> bool {
        let r = self.rect[i];
        if !(p.0 >= r[0] && p.0 <= r[2] && p.1 >= r[1] && p.1 <= r[3]) {
            return false;
        }
        let ring = self.outline(i);
        let mut inside = false;
        for j in 0..ring.len() {
            let (a, b) = (ring[j], ring[(j + 1) % ring.len()]);
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                inside = !inside;
            }
        }
        inside
    }

    /// The smallest block containing `p`, so blocks of a road network inside another block win.
    pub fn at(&self, p: Point) -> Option<usize> {
        (0..self.len())
            .filter(|&i| self.contains(i, p))
            .min_by(|&i, &j| self.area[i].total_cmp(&self.area[j]).then(i.cmp(&j)))
    }

    /// Block `i`'s cells: inside it, not inside a smaller block within it, and not under a road.
    pub(crate) fn cells(&self, i: usize, grid: &ZoneGrid) -> Vec<Cell> {
        let rect = self.rect[i];
        let nested = self.nested(i);
        let roads = self.roads_near(rect);
        let mut cells = Vec::new();
        for index in grid.cells_in(rect) {
            let at = grid.centre(index);
            if !self.contains(i, at) || nested.iter().any(|&j| self.contains(j, at)) {
                continue;
            }
            let Some(cell) = self.nearest(&roads, index, at) else {
                continue;
            };
            if cell.dist >= ROAD_HALF_WIDTH {
                cells.push(cell);
            }
        }
        cells
    }

    /// Smaller blocks inside block `i`'s bounds: candidates for blocks within it.
    pub(super) fn nested(&self, i: usize) -> Vec<usize> {
        (0..self.len())
            .filter(|&j| {
                j != i
                    && self.area[j] < self.area[i]
                    && contains_rect(self.rect[i], self.rect[j], 0.0)
            })
            .collect()
    }

    pub(super) fn roads_near(&self, rect: Rect) -> Vec<RoadId> {
        (0..self.road_rect.len() as RoadId)
            .filter(|&r| intersects(self.road_rect[r as usize], rect))
            .collect()
    }

    /// The road whose centreline passes nearest `p`, among roads whose bounds are within a
    /// metre of it.
    pub(crate) fn road_at(&self, p: Point) -> Option<RoadId> {
        let near = [p.0 - 1.0, p.1 - 1.0, p.0 + 1.0, p.1 + 1.0];
        let roads: Vec<RoadId> = (0..self.road_rect.len() as RoadId)
            .filter(|&r| intersects(self.road_rect[r as usize], near))
            .collect();
        self.nearest(&roads, 0, p).map(|c| c.road)
    }

    fn nearest(&self, roads: &[RoadId], index: u32, at: Point) -> Option<Cell> {
        let mut best: Option<Cell> = None;
        for &r in roads {
            let pts = self.road_points(r);
            for k in 0..pts.len() - 1 {
                let (a, b) = (pts[k], pts[k + 1]);
                let f = project(a, b, at);
                let d = dist(lerp(a, b, f), at);
                if best.is_none_or(|c| d < c.dist) {
                    best = Some(Cell {
                        index,
                        at,
                        dist: d,
                        road: r,
                    });
                }
            }
        }
        best
    }
}
