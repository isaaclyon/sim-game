//! Lots: the pieces of blocks that buildings will stand on (ADR 0016).
//!
//! A lot is a set of zone cells facing one road. Ids are append-only; a retired lot keeps its
//! slot and record, like a deleted road (ADR 0013).

use crate::block::{LotPlan, Rect, intersects};
use crate::road::RoadId;
use crate::zone::{Zone, ZoneGrid};

pub type LotId = u32;
const NO_LOT: LotId = LotId::MAX;

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

    /// Active lots cut from blocks whose bounding box meets `rect`.
    pub(crate) fn active_in(&self, rect: Rect) -> Vec<LotId> {
        self.active_lots()
            .filter(|&l| intersects(self.block[l as usize], rect))
            .collect()
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
    pub(crate) fn push(&mut self, plan: LotPlan, block: Rect, grid_cells: usize) -> bool {
        if self.owner.is_empty() {
            self.owner = vec![NO_LOT; grid_cells];
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
        self.active.push(true);
        self.block.push(block);
        self.cells.extend_from_slice(&plan.cells);
        self.cell_start.push(self.cells.len() as u32);
        true
    }

    /// Writes `[ax, ay, bx, by]` per outline segment: zone cell edges where the owning lot
    /// changes and at least one side is a lot, merged into straight runs.
    pub fn write_outlines(&self, grid: &ZoneGrid, out: &mut Vec<f32>) {
        out.clear();
        if self.owner.is_empty() {
            return;
        }
        let (cols, rows) = (grid.cols as usize, grid.rows as usize);
        let size = crate::zone::ZONE_CELL;
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
