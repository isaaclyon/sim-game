//! What the undo and redo buttons show (ADR 0051): read-only, not hashed.

use super::{Record, StepKind, pieces};
use crate::world::World;

/// The steps undo and redo would take, and what they would cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryView {
    pub undo: Option<StepKind>,
    pub redo: Option<StepKind>,
    /// Dollars undo would give back; None with nothing to undo.
    pub undo_refund: Option<i64>,
    /// Dollars redo would charge; None with nothing to redo.
    pub redo_cost: Option<i64>,
    /// False when the city cannot pay for the redo, which would then be refused.
    pub redo_affordable: bool,
}

impl World {
    pub fn history_view(&self) -> HistoryView {
        let (undo, redo) = (self.history.undo_step(), self.history.redo_step());
        HistoryView {
            undo: undo.map(|s| s.kind),
            redo: redo.map(|s| s.kind),
            undo_refund: undo.map(|s| s.cost),
            redo_cost: redo.map(|s| s.cost),
            redo_affordable: redo.is_some_and(|s| self.can_afford(s.cost)),
        }
    }

    /// Writes `[n, x0, y0, …]` per record that undo (or `redo`) would remove and restore: a
    /// road's centreline samples, a plant's, pump's, tower's or station's closed outline, or a
    /// line's two ends. Returns whether it would succeed now. Changes nothing.
    pub fn write_history_preview(
        &self,
        redo: bool,
        remove: &mut Vec<f32>,
        restore: &mut Vec<f32>,
    ) -> bool {
        remove.clear();
        restore.clear();
        let step = if redo {
            self.history.redo_step()
        } else {
            self.history.undo_step()
        };
        let Some(step) = step else {
            return false;
        };
        let (gone, back) = if redo {
            (&step.before, &step.after)
        } else {
            (&step.after, &step.before)
        };
        for (list, out) in [(gone, &mut *remove), (back, &mut *restore)] {
            for &r in list {
                write_record(r, out);
            }
        }
        self.step_fits(gone, back) && (!redo || self.can_afford(step.cost))
    }
}

fn write_record(record: Record, out: &mut Vec<f32>) {
    let at = out.len();
    out.push(0.0);
    match record {
        Record::Road(r) => {
            let shape = r.shape();
            out.extend([shape.a.0, shape.a.1]);
            for (_, _, _, p) in shape.segments() {
                out.extend([p.0, p.1]);
            }
        }
        Record::Line { a, b } => out.extend([a.0, a.1, b.0, b.1]),
        _ => {
            let ring = pieces::footprint(record).expect("a piece").ring();
            out.extend(ring.iter().chain(&ring[..1]).flat_map(|p| [p.0, p.1]));
        }
    }
    out[at] = ((out.len() - at - 1) / 2) as f32;
}
