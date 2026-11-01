//! Lots: the pieces of blocks that buildings will stand on (ADR 0016, ADR 0017).
//!
//! A lot is a polygon cut along one road, plus the zone cells whose centre lies inside it.
//! Ids are append-only; a retired lot keeps its slot and record, like a deleted road
//! (ADR 0013).

use crate::block::{LotPlan, Rect};
use crate::geometry::Point;
use crate::road::RoadId;
use crate::zone::{Zone, ZoneGrid};

/// A recut lot whose outline points all lie this close to an old lot's keeps its id, metres.
const SAME_OUTLINE: f32 = 0.01;

pub type LotId = u32;
const NO_LOT: LotId = LotId::MAX;

#[derive(Default)]
pub struct Lots {
    /// The road the lot faces.
    pub road: Vec<RoadId>,
    /// Length of the stretch of road the lot was cut from, metres.
    pub frontage: Vec<f32>,
    /// Centreline point at the middle of the lot's stretch of road.
    pub front_x: Vec<f32>,
    pub front_y: Vec<f32>,
    /// Unit direction from the lot toward its road.
    pub facing_x: Vec<f32>,
    pub facing_y: Vec<f32>,
    pub active: Vec<bool>,
    /// Bounding box of the block the lot was cut from.
    block: Vec<Rect>,
    // CSR: lot l's outline is outline[outline_start[l]..outline_start[l + 1]].
    outline_start: Vec<u32>,
    outline: Vec<Point>,
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

    /// `lot`'s outline, counter-clockwise, without repeating the first point.
    pub fn outline(&self, lot: LotId) -> &[Point] {
        let l = lot as usize;
        &self.outline[self.outline_start[l] as usize..self.outline_start[l + 1] as usize]
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

    /// The active lot identical to `plan`, if any: same road, front point and cells, and an
    /// outline the same to within `SAME_OUTLINE`.
    pub(crate) fn matching(&self, plan: &LotPlan) -> Option<LotId> {
        let lot = self.owner(*plan.cells.first()?)?;
        let l = lot as usize;
        let old = self.outline(lot);
        (self.road[l] == plan.road
            && (self.front_x[l], self.front_y[l]) == plan.front
            && self.cells(lot) == plan.cells
            && old.len() == plan.outline.len()
            && old.iter().zip(&plan.outline).all(|(a, b)| {
                (a.0 - b.0).abs() <= SAME_OUTLINE && (a.1 - b.1).abs() <= SAME_OUTLINE
            }))
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
            self.outline_start.push(0);
        }
        if plan.cells.iter().any(|&c| self.owner[c as usize] != NO_LOT) {
            return false;
        }
        let id = self.active.len() as LotId;
        for &c in &plan.cells {
            self.owner[c as usize] = id;
        }
        self.road.push(plan.road);
        self.frontage.push(plan.frontage);
        self.front_x.push(plan.front.0);
        self.front_y.push(plan.front.1);
        self.facing_x.push(plan.facing.0);
        self.facing_y.push(plan.facing.1);
        self.active.push(true);
        self.block.push(block);
        self.outline.extend_from_slice(&plan.outline);
        self.outline_start.push(self.outline.len() as u32);
        self.cells.extend_from_slice(&plan.cells);
        self.cell_start.push(self.cells.len() as u32);
        true
    }

    /// Writes `[n, x0, y0, …, x(n-1), y(n-1)]` per active lot: its outline's point count, then
    /// its points counter-clockwise.
    pub fn write_outlines(&self, out: &mut Vec<f32>) {
        out.clear();
        for lot in self.active_lots() {
            let ring = self.outline(lot);
            out.push(ring.len() as f32);
            out.extend(ring.iter().flat_map(|p| [p.0, p.1]));
        }
    }
}
