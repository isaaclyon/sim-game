//! Undo and redo (ADR 0051): apply a step backwards or forwards as a new edit, all or nothing.

use super::{Capture, Record, Step, roads};
use crate::world::World;
use crate::world::zoning::Scope;

impl World {
    /// Takes back the newest undo step, refunding its cost. Refused when there is none or it
    /// no longer fits the map; the lists then stay as they are.
    pub(in crate::world) fn undo(&mut self) -> bool {
        let Some(step) = self.history.undo.last().cloned() else {
            return false;
        };
        let capture = Capture::of(self);
        if !self.replay_step(&step.after, &step.before) {
            return false;
        }
        self.treasury.balance += step.cost;
        let seen = capture.diff(self);
        let h = &mut self.history;
        h.undo.pop();
        // Redo reverses what undo really did, even where a join or alley made it differ.
        h.redo.push(Step {
            kind: step.kind,
            before: seen.after,
            after: seen.before,
            cost: step.cost,
        });
        h.chain_open = false;
        h.trim(true);
        true
    }

    /// Makes the newest undone step again, charging its cost. Refused when there is none, it
    /// no longer fits the map, or the city cannot pay.
    pub(in crate::world) fn redo(&mut self) -> bool {
        let Some(step) = self.history.redo.last().cloned() else {
            return false;
        };
        if !self.can_afford(step.cost) {
            return false;
        }
        let capture = Capture::of(self);
        if !self.replay_step(&step.before, &step.after) {
            return false;
        }
        self.treasury.balance -= step.cost;
        let seen = capture.diff(self);
        let h = &mut self.history;
        h.redo.pop();
        h.undo.push(Step {
            kind: step.kind,
            before: seen.before,
            after: seen.after,
            cost: step.cost,
        });
        h.chain_open = false;
        h.trim(false);
        true
    }

    /// Whether removing `remove` and restoring `restore` would succeed now. Changes nothing.
    pub(in crate::world) fn step_fits(&self, remove: &[Record], restore: &[Record]) -> bool {
        if self.find_pieces(remove).is_none() {
            return false;
        }
        let mut net = self.roads.clone();
        roads::replay(&mut net, remove, restore, &mut |_, _| {}).is_some()
            && self.pieces_fit(&net, restore)
    }

    /// Removes `remove` and restores `restore`, then tidies up as the edits that placed them
    /// do. Refused, changing nothing, unless `step_fits`.
    fn replay_step(&mut self, remove: &[Record], restore: &[Record]) -> bool {
        if !self.step_fits(remove, restore) {
            return false;
        }
        let pieces = self.find_pieces(remove).expect("checked by step_fits");
        if let Some(rect) = roads::area(remove, restore) {
            let mut changed = Vec::new();
            let junctions = &mut self.junctions;
            let removed = roads::replay(&mut self.roads, remove, restore, &mut |road, rest| {
                junctions.remap_split(road, rest);
                changed.push(road);
            })
            .expect("checked by step_fits");
            changed.extend(removed);
            changed.extend(self.join_straight_roads());
            self.sync_roads();
            self.remove_cars_using(&changed);
            self.update_lots(Scope::Near(rect), true);
        }
        for piece in pieces {
            self.remove_piece(piece);
        }
        for &record in restore {
            self.restore_piece(record);
        }
        true
    }
}
