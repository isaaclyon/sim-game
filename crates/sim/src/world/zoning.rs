//! Zoning commands and the alleys and lots that follow them.

use std::collections::BTreeSet;

use super::World;
use crate::block::{self, Blocks, LOT_REACH, MIN_DEEP_CELLS, Rect};
use crate::road::{RoadClass, RoadId};
use crate::zone::Zone;

/// Rounds of alley cuts one zoning command may make; each round can halve a deep block.
const ALLEY_ROUNDS: u32 = 6;

/// Which blocks a lot update looks at, by bounding box.
pub(super) enum Scope<'a> {
    /// Blocks meeting this area: after a player's edit.
    Near(Rect),
    /// Blocks inside one of these blocks' bounds: the pieces of blocks just divided by alleys.
    Inside(&'a [Rect]),
}

impl Scope<'_> {
    fn has(&self, block: Rect) -> bool {
        match self {
            Scope::Near(area) => block::intersects(block, *area),
            Scope::Inside(blocks) => blocks.iter().any(|&b| block::contains_rect(b, block, 0.5)),
        }
    }
}

impl World {
    /// Sets the cells of the block containing `(x, y)`. See ADR 0016.
    pub(super) fn fill_block(&mut self, x: f32, y: f32, zone: Zone) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        self.refresh_blocks();
        let Some(b) = self.blocks.at((x, y)) else {
            return false;
        };
        let cells: Vec<u32> = self
            .blocks
            .cells(b, &self.zones)
            .iter()
            .map(|c| c.index)
            .collect();
        let changed = self.zones.set(&cells, zone);
        if changed.is_empty() {
            return false;
        }
        self.after_zoning(self.blocks.rect(b), &changed);
        true
    }

    /// After a zoning command changed cells: alleys for deep blocks, then lots for newly zoned
    /// blocks. `changed` holds the cells whose zone changed, ascending; `rect` bounds them.
    pub(super) fn after_zoning(&mut self, rect: Rect, changed: &[u32]) {
        let (divided, split) = self.add_alleys(rect, changed);
        if !divided.is_empty() {
            // Lots across the street from an alley keep their ids; the road they front was
            // only split, so they move to the piece in front of them.
            self.refresh_blocks();
            for lot in self.lots.active_lots().collect::<Vec<_>>() {
                let l = lot as usize;
                if split.contains(&self.lots.road[l])
                    && let Some(road) = self
                        .blocks
                        .road_at((self.lots.front_x[l], self.lots.front_y[l]))
                {
                    self.lots.road[l] = road;
                }
            }
            self.update_lots(Scope::Inside(&divided), true);
        }
        self.update_lots(Scope::Near(rect), false);
    }

    /// Cuts blocks near `rect` that have enough zoned cells out of a road's reach, some of them
    /// just changed, with alleys or connector streets; then the pieces, for a few rounds.
    /// Returns the bounds of the blocks it divided and the roads it split.
    fn add_alleys(&mut self, rect: Rect, changed: &[u32]) -> (Vec<Rect>, Vec<RoadId>) {
        let mut divided = Vec::new();
        let mut split = Vec::new();
        let mut cut: Vec<Rect> = Vec::new();
        for round in 0..ALLEY_ROUNDS {
            self.refresh_blocks();
            let mut plans = Vec::new();
            for b in 0..self.blocks.len() {
                let br = self.blocks.rect(b);
                let eligible = if round == 0 {
                    block::intersects(br, rect)
                } else {
                    cut.iter().any(|&r| block::contains_rect(r, br, 0.5))
                };
                if !eligible {
                    continue;
                }
                let cells = self.blocks.cells(b, &self.zones);
                let deep: Vec<u32> = cells
                    .iter()
                    .filter(|c| c.dist > LOT_REACH && self.zones.zone(c.index) != Zone::None)
                    .map(|c| c.index)
                    .collect();
                if deep.len() < MIN_DEEP_CELLS
                    || (round == 0 && !deep.iter().any(|c| changed.binary_search(c).is_ok()))
                {
                    continue;
                }
                plans.push((br, self.blocks.cuts(b, &cells, &self.zones)));
            }
            cut.clear();
            for (br, options) in plans {
                for (shape, alley) in options {
                    let class = if alley {
                        RoadClass::Alley
                    } else {
                        RoadClass::Street
                    };
                    if self.add_road(shape, class, &mut split).is_some() {
                        if round == 0 {
                            divided.push(br);
                        }
                        cut.push(br);
                        break;
                    }
                }
            }
            if cut.is_empty() {
                break;
            }
        }
        if !divided.is_empty() {
            self.sync_roads();
            self.remove_cars_using(&split);
        }
        (divided, split)
    }

    /// Recuts lots in the blocks of `scope` (ADR 0016, ADR 0018). With `retire`, after a road
    /// edit: lots there that no longer match are retired. Either way, new lots are added where
    /// at least half their cells are zoned and no lot holds them yet.
    pub(super) fn update_lots(&mut self, scope: Scope, retire: bool) {
        if self.lots.active_lots().next().is_none() && !self.zones.any_zoned() {
            return;
        }
        self.refresh_blocks();
        let old = if retire {
            self.lots.active_in(|r| scope.has(r))
        } else {
            Vec::new()
        };
        let mut kept = BTreeSet::new();
        let mut plans = Vec::new();
        for b in 0..self.blocks.len() {
            let br = self.blocks.rect(b);
            if !scope.has(br) {
                continue;
            }
            let cells = self.blocks.cells(b, &self.zones);
            let has_lots = cells.iter().any(|c| self.lots.owner(c.index).is_some());
            if !has_lots && cells.iter().all(|c| self.zones.zone(c.index) == Zone::None) {
                continue;
            }
            for plan in self.blocks.cut(b, &self.zones) {
                match self.lots.matching(&plan) {
                    Some(lot) => {
                        // Its block may have grown or shrunk around it.
                        self.lots.set_block(lot, br);
                        kept.insert(lot);
                    }
                    None => {
                        let zoned = plan
                            .cells
                            .iter()
                            .filter(|&&c| self.zones.zone(c) != Zone::None)
                            .count();
                        if zoned > 0 && 2 * zoned >= plan.cells.len() {
                            plans.push((plan, br));
                        }
                    }
                }
            }
        }
        let mut changed = false;
        for lot in old {
            if !kept.contains(&lot) {
                self.lots.retire(lot);
                changed = true;
            }
        }
        for (plan, br) in plans {
            changed |= self.lots.push(plan, br, self.zones.cells.len());
        }
        if changed {
            self.lots.version = self.lots.version.wrapping_add(1);
        }
    }

    /// Traces the road network's blocks again if roads changed since the last time.
    pub(super) fn refresh_blocks(&mut self) {
        if self.blocks_version == Some(self.roads.version) {
            return;
        }
        self.roads.rebuild_adjacency();
        self.blocks = Blocks::find(&self.roads);
        self.blocks_version = Some(self.roads.version);
    }
}
