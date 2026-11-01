//! Lots: the pieces of blocks that buildings will stand on (ADR 0016).
//!
//! A lot is a set of zone cells facing one road. Ids are append-only; a retired lot keeps its
//! slot and record, like a deleted road (ADR 0013).

use crate::block::{LotPlan, Rect};
use crate::geometry::Point;
use crate::road::RoadId;
use crate::zone::{ZONE_CELL, Zone, ZoneGrid};

pub type LotId = u32;
const NO_LOT: LotId = LotId::MAX;
/// Building areas are found on a grid this fine, metres, so they fit their lot to within it.
const AREA_STEP: f32 = 2.0;
/// A building area is at least this wide and deep, metres; smaller lots have none.
pub const MIN_AREA_SIDE: f32 = 8.0;

/// The largest rectangle facing `facing` inside the union of `cells`, found on an `AREA_STEP`
/// grid: `(centre, width along the road, depth toward it)`. `None` if no rectangle of at least
/// `MIN_AREA_SIDE` by `MIN_AREA_SIDE` fits. Ties go to the rectangle nearest the road.
pub(crate) fn building_area(
    cells: &[u32],
    facing: Point,
    grid: &ZoneGrid,
) -> Option<(Point, f32, f32)> {
    let cols = grid.cols;
    let (c0, c1) = cells.iter().fold((u32::MAX, 0), |(lo, hi), &c| {
        (lo.min(c % cols), hi.max(c % cols))
    });
    let (r0, r1) = (cells[0] / cols, cells[cells.len() - 1] / cols);
    let w = (c1 - c0 + 1) as usize;
    let mut mine = vec![false; w * (r1 - r0 + 1) as usize];
    for &c in cells {
        mine[(c / cols - r0) as usize * w + (c % cols - c0) as usize] = true;
    }
    // Lot-local coordinates, relative to the first cell's corner, keep the arithmetic exact.
    let origin = (
        grid.x0 + c0 as f32 * ZONE_CELL,
        grid.y0 + r0 as f32 * ZONE_CELL,
    );
    let inside = |x: f32, y: f32| {
        let (c, r) = ((x / ZONE_CELL).floor(), (y / ZONE_CELL).floor());
        c >= 0.0
            && r >= 0.0
            && (c as usize) < w
            && (r as u32) <= r1 - r0
            && mine[r as usize * w + c as usize]
    };
    // Frame: v toward the road, u along it.
    let v = facing;
    let u = (-v.1, v.0);
    let (mut a0, mut a1, mut b0, mut b1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &c in cells {
        let (x, y) = (
            (c % cols - c0) as f32 * ZONE_CELL,
            (c / cols - r0) as f32 * ZONE_CELL,
        );
        for (px, py) in [
            (x, y),
            (x + ZONE_CELL, y),
            (x, y + ZONE_CELL),
            (x + ZONE_CELL, y + ZONE_CELL),
        ] {
            let (a, b) = (px * u.0 + py * u.1, px * v.0 + py * v.1);
            (a0, a1, b0, b1) = (a0.min(a), a1.max(a), b0.min(b), b1.max(b));
        }
    }
    let na = ((a1 - a0) / AREA_STEP - 1e-3).ceil().max(0.0) as usize;
    let nb = ((b1 - b0) / AREA_STEP - 1e-3).ceil().max(0.0) as usize;
    let at = |a: f32, b: f32| (u.0 * a + v.0 * b, u.1 * a + v.1 * b);
    // A grid square counts if its centre and its corners, pulled in slightly, are in the lot,
    // and so are the four cells around any cell corner inside it. Cell edges are longer than
    // the square, so an edge can only cut into it through such a corner.
    let square = |i: usize, j: usize| {
        let (a, b) = (a0 + i as f32 * AREA_STEP, b0 + j as f32 * AREA_STEP);
        let (lo, hi, mid) = (0.01, AREA_STEP - 0.01, AREA_STEP / 2.0);
        let corners = [(mid, mid), (lo, lo), (hi, lo), (lo, hi), (hi, hi)]
            .iter()
            .all(|&(da, db)| {
                let (x, y) = at(a + da, b + db);
                inside(x, y)
            });
        let (x, y) = at(a + mid, b + mid);
        let (lx, ly) = (
            (x / ZONE_CELL).round() * ZONE_CELL,
            (y / ZONE_CELL).round() * ZONE_CELL,
        );
        let (la, lb) = (lx * u.0 + ly * u.1 - a, lx * v.0 + ly * v.1 - b);
        let corner_in = la > 0.0 && la < AREA_STEP && lb > 0.0 && lb < AREA_STEP;
        corners
            && (!corner_in
                || [(-0.01, -0.01), (0.01, -0.01), (-0.01, 0.01), (0.01, 0.01)]
                    .iter()
                    .all(|&(dx, dy)| inside(lx + dx, ly + dy)))
    };
    // Largest rectangle of counted squares: per row of b, column heights grow away from b0.
    let min = (MIN_AREA_SIDE / AREA_STEP).round() as usize;
    let mut heights = vec![0usize; na];
    // Best so far: (area, twice its centre row, reversed left) to compare, and
    // (left, right, top, height).
    let mut best: Option<(usize, usize, usize)> = None;
    let mut best_rect = (0, 0, 0, 0);
    let mut stack: Vec<usize> = Vec::new();
    for j in 0..nb {
        for (i, h) in heights.iter_mut().enumerate() {
            *h = if square(i, j) { *h + 1 } else { 0 };
        }
        stack.clear();
        for i in 0..=na {
            let h = if i < na { heights[i] } else { 0 };
            while let Some(&k) = stack.last() {
                if heights[k] < h {
                    break;
                }
                stack.pop();
                let left = stack.last().map_or(0, |&s| s + 1);
                let (width, height) = (i - left, heights[k]);
                if width >= min && height >= min {
                    // Larger first, then centred nearer the road (higher row), then lower along it.
                    let key = (width * height, 2 * j + 2 - height, usize::MAX - left);
                    if best.is_none_or(|b| key > b) {
                        best = Some(key);
                        best_rect = (left, i, j, height);
                    }
                }
            }
            stack.push(i);
        }
    }
    best?;
    let (left, right, top, height) = best_rect;
    let (width, depth) = ((right - left) as f32 * AREA_STEP, height as f32 * AREA_STEP);
    let mid_a = a0 + (left + right) as f32 * AREA_STEP / 2.0;
    let mid_b = b0 + (top + 1) as f32 * AREA_STEP - depth / 2.0;
    let (x, y) = at(mid_a, mid_b);
    Some(((origin.0 + x, origin.1 + y), width, depth))
}

#[derive(Default)]
pub struct Lots {
    /// The road the lot faces.
    pub road: Vec<RoadId>,
    /// Centreline point at the middle of the lot's stretch of road.
    pub front_x: Vec<f32>,
    pub front_y: Vec<f32>,
    /// Unit direction from the lot toward its road.
    pub facing_x: Vec<f32>,
    pub facing_y: Vec<f32>,
    pub active: Vec<bool>,
    /// Where a building can stand: the centre of the largest rectangle in the lot that faces
    /// its road, its width along the road and depth toward it. Width 0 = no building area.
    pub area_x: Vec<f32>,
    pub area_y: Vec<f32>,
    pub area_w: Vec<f32>,
    pub area_d: Vec<f32>,
    /// Bounding box of the block the lot was cut from.
    block: Vec<Rect>,
    // CSR: lot l owns cells[cell_start[l]..cell_start[l + 1]].
    cell_start: Vec<u32>,
    cells: Vec<u32>,
    /// Owning lot per zone cell, `NO_LOT` if none. Empty until the first lot is cut.
    owner: Vec<LotId>,
    /// Bumped on every change so the frontend knows when to redraw.
    pub version: u32,
}

impl Lots {
    /// Allocated lot slots, including retired ones.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    pub fn active_lots(&self) -> impl Iterator<Item = LotId> + '_ {
        self.active
            .iter()
            .enumerate()
            .filter_map(|(l, &on)| on.then_some(l as LotId))
    }

    /// Row-major zone cell indices of `lot`, ascending.
    pub fn cells(&self, lot: LotId) -> &[u32] {
        let l = lot as usize;
        &self.cells[self.cell_start[l] as usize..self.cell_start[l + 1] as usize]
    }

    /// The active lot owning zone cell `index`.
    pub fn owner(&self, index: u32) -> Option<LotId> {
        self.owner
            .get(index as usize)
            .copied()
            .filter(|&l| l != NO_LOT)
    }

    /// The most common zone among the lot's cells; ties go to the lower zone value.
    pub fn zone(&self, lot: LotId, grid: &ZoneGrid) -> Zone {
        let mut counts = [0u32; 7];
        for &c in self.cells(lot) {
            counts[grid.zone(c) as usize] += 1;
        }
        let best = (0..counts.len()).fold(0, |b, z| if counts[z] > counts[b] { z } else { b });
        Zone::from_u8(best as u8).unwrap_or(Zone::None)
    }

    /// Active lots whose block bounds pass `hit`.
    pub(crate) fn active_in(&self, hit: impl Fn(Rect) -> bool) -> Vec<LotId> {
        self.active_lots()
            .filter(|&l| hit(self.block[l as usize]))
            .collect()
    }

    pub(crate) fn set_block(&mut self, lot: LotId, block: Rect) {
        self.block[lot as usize] = block;
    }

    /// The active lot identical to `plan`, if any.
    pub(crate) fn matching(&self, plan: &LotPlan) -> Option<LotId> {
        let lot = self.owner(plan.cells[0])?;
        let l = lot as usize;
        (self.road[l] == plan.road
            && (self.front_x[l], self.front_y[l]) == plan.front
            && self.cells(lot) == plan.cells)
            .then_some(lot)
    }

    pub(crate) fn retire(&mut self, lot: LotId) {
        self.active[lot as usize] = false;
        let (start, end) = (
            self.cell_start[lot as usize] as usize,
            self.cell_start[lot as usize + 1] as usize,
        );
        for i in start..end {
            let cell = self.cells[i] as usize;
            if self.owner[cell] == lot {
                self.owner[cell] = NO_LOT;
            }
        }
    }

    /// Appends a lot unless one of its cells already has one. Returns whether it was added.
    pub(crate) fn push(&mut self, plan: LotPlan, block: Rect, grid: &ZoneGrid) -> bool {
        if self.owner.is_empty() {
            self.owner = vec![NO_LOT; grid.cells.len()];
        }
        if self.cell_start.is_empty() {
            self.cell_start.push(0);
        }
        if plan.cells.iter().any(|&c| self.owner[c as usize] != NO_LOT) {
            return false;
        }
        let id = self.active.len() as LotId;
        for &c in &plan.cells {
            self.owner[c as usize] = id;
        }
        self.road.push(plan.road);
        self.front_x.push(plan.front.0);
        self.front_y.push(plan.front.1);
        self.facing_x.push(plan.facing.0);
        self.facing_y.push(plan.facing.1);
        let (centre, w, d) =
            building_area(&plan.cells, plan.facing, grid).unwrap_or(((0.0, 0.0), 0.0, 0.0));
        self.area_x.push(centre.0);
        self.area_y.push(centre.1);
        self.area_w.push(w);
        self.area_d.push(d);
        self.active.push(true);
        self.block.push(block);
        self.cells.extend_from_slice(&plan.cells);
        self.cell_start.push(self.cells.len() as u32);
        true
    }

    /// Writes `[ax, ay, bx, by]` per edge of each active lot's building area.
    pub fn write_building_areas(&self, out: &mut Vec<f32>) {
        out.clear();
        for lot in self.active_lots() {
            let l = lot as usize;
            if self.area_w[l] == 0.0 {
                continue;
            }
            let (vx, vy) = (self.facing_x[l], self.facing_y[l]);
            let (hw, hd) = (self.area_w[l] / 2.0, self.area_d[l] / 2.0);
            let corner = |su: f32, sv: f32| {
                (
                    self.area_x[l] - vy * hw * su + vx * hd * sv,
                    self.area_y[l] + vx * hw * su + vy * hd * sv,
                )
            };
            let c = [
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ];
            for k in 0..4 {
                let (a, b) = (c[k], c[(k + 1) % 4]);
                out.extend([a.0, a.1, b.0, b.1]);
            }
        }
    }

    /// Writes `[ax, ay, bx, by]` per outline segment: zone cell edges where the owning lot
    /// changes and at least one side is a lot, merged into straight runs.
    pub fn write_outlines(&self, grid: &ZoneGrid, out: &mut Vec<f32>) {
        out.clear();
        if self.owner.is_empty() {
            return;
        }
        let (cols, rows) = (grid.cols as usize, grid.rows as usize);
        let size = ZONE_CELL;
        let at = |c: usize, r: usize| -> LotId {
            if c < cols && r < rows {
                self.owner[r * cols + c]
            } else {
                NO_LOT
            }
        };
        let edge = |a: LotId, b: LotId| a != b;
        // Horizontal edges: the line y = y0 + r * size, between rows r - 1 and r.
        for r in 0..=rows {
            let mut run: Option<usize> = None;
            for c in 0..=cols {
                let below = if r > 0 { at(c, r - 1) } else { NO_LOT };
                let on = c < cols && edge(below, at(c, r));
                match (on, run) {
                    (true, None) => run = Some(c),
                    (false, Some(s)) => {
                        let y = grid.y0 + r as f32 * size;
                        out.extend([grid.x0 + s as f32 * size, y, grid.x0 + c as f32 * size, y]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
        // Vertical edges: the line x = x0 + c * size, between columns c - 1 and c.
        for c in 0..=cols {
            let mut run: Option<usize> = None;
            for r in 0..=rows {
                let left = if c > 0 { at(c - 1, r) } else { NO_LOT };
                let on = r < rows && edge(left, at(c, r));
                match (on, run) {
                    (true, None) => run = Some(r),
                    (false, Some(s)) => {
                        let x = grid.x0 + c as f32 * size;
                        out.extend([x, grid.y0 + s as f32 * size, x, grid.y0 + r as f32 * size]);
                        run = None;
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Row-major indices of the cells at `(col, row)`, ascending.
    fn cells(g: &ZoneGrid, at: &[(u32, u32)]) -> Vec<u32> {
        let mut v: Vec<u32> = at.iter().map(|&(c, r)| r * g.cols + c).collect();
        v.sort_unstable();
        v
    }

    fn grid() -> ZoneGrid {
        ZoneGrid::covering([0.0, 0.0, 100.0, 100.0])
    }

    #[test]
    fn a_rectangular_lot_is_all_building_area() {
        let g = grid();
        // Two cells wide, four deep, facing down (-y) toward a road along x.
        let lot: Vec<(u32, u32)> = (0..2).flat_map(|c| (0..4).map(move |r| (c, r))).collect();
        let (centre, w, d) = building_area(&cells(&g, &lot), (0.0, -1.0), &g).unwrap();
        assert_eq!((w, d), (16.0, 32.0));
        assert_eq!(centre, (g.x0 + 8.0, g.y0 + 16.0));
    }

    #[test]
    fn an_l_shaped_lot_gets_its_biggest_rectangle_nearest_the_road() {
        let g = grid();
        // A 3 x 1 row along the road (y = 0 side) and a 1 x 3 arm going back from one end.
        let lot = cells(&g, &[(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)]);
        let (centre, w, d) = building_area(&lot, (0.0, -1.0), &g).unwrap();
        // 24 x 8 along the road and 8 x 24 back from it tie; the one nearer the road wins.
        assert_eq!((w, d), (24.0, 8.0));
        assert_eq!(centre, (g.x0 + 12.0, g.y0 + 4.0));
    }

    #[test]
    fn lots_too_small_for_their_road_angle_have_no_building_area() {
        let g = grid();
        // A single cell fits exactly; a road at 45 degrees leaves no 8 x 8 square inside it.
        let one = cells(&g, &[(5, 5)]);
        assert!(building_area(&one, (0.0, -1.0), &g).is_some());
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert!(building_area(&one, (s, -s), &g).is_none());
    }

    #[test]
    fn building_areas_turn_with_their_road_and_stay_inside() {
        let g = grid();
        let block: Vec<(u32, u32)> = (0..4).flat_map(|c| (0..4).map(move |r| (c, r))).collect();
        let lot = cells(&g, &block);
        let (s, c) = (0.5f32, 0.75f32.sqrt()); // 30 degrees.
        let (centre, w, d) = building_area(&lot, (s, -c), &g).unwrap();
        assert!(w >= MIN_AREA_SIDE && d >= MIN_AREA_SIDE);
        // Its corners lie within the 32 x 32 m square.
        let u = (c, s);
        for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let x = centre.0 + u.0 * w / 2.0 * su + s * d / 2.0 * sv - g.x0;
            let y = centre.1 + u.1 * w / 2.0 * su - c * d / 2.0 * sv - g.y0;
            assert!(
                (-0.01..=32.01).contains(&x) && (-0.01..=32.01).contains(&y),
                "({x}, {y})"
            );
        }
        // About half a rotated square's area fits; the grid loses a little at the edges.
        assert!(w * d > 400.0, "{w} x {d}");
    }
}
