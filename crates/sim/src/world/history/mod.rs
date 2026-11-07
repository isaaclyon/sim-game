//! Edit history (ADR 0051): undo and redo for roads, power plants and lines, pumps, towers and
//! fire stations. A step records what an edit removed (`before`) and added (`after`) as shapes
//! and footprints, never IDs; undo and redo replay them as new edits with new IDs.

mod apply;
mod capture;
mod pieces;
mod roads;
mod view;

use serde::{Deserialize, Serialize};

use super::{Command, World};
use crate::check::{Check, ensure};
use crate::geometry::{Point, Shape};
use crate::power::PlantKind;
use crate::road::RoadClass;
use crate::water::WaterKind;

pub(super) use capture::Capture;
pub use view::HistoryView;

/// At most this many steps, undo and redo together; the oldest undo steps go first.
pub const MAX_STEPS: usize = 100;
/// At most this many records across all steps; the oldest undo steps go first.
pub const MAX_RECORDS: usize = 10_000;

/// A road as it stood: its ends, a curve's control point and its class.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoadRecord {
    pub a: Point,
    pub b: Point,
    pub control: Option<Point>,
    pub class: RoadClass,
}

impl RoadRecord {
    pub(crate) fn shape(self) -> Shape {
        Shape {
            a: self.a,
            b: self.b,
            control: self.control,
        }
    }

    /// Same position, whatever the class: a class change, applied in place.
    fn same_place(self, other: Self) -> bool {
        self.a == other.a && self.b == other.b && self.control == other.control
    }
}

/// One placeable as it stood. Plants, pumps, towers and stations keep their placed footprint
/// (centre and the unit direction their front faces), lines their snapped ends.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Record {
    Road(RoadRecord),
    Plant {
        kind: PlantKind,
        centre: Point,
        front: Point,
    },
    Line {
        a: Point,
        b: Point,
    },
    Water {
        kind: WaterKind,
        centre: Point,
        front: Point,
    },
    Station {
        centre: Point,
        front: Point,
    },
}

impl Record {
    fn points(&self) -> Vec<Point> {
        match *self {
            Self::Road(r) => [r.a, r.b].into_iter().chain(r.control).collect(),
            Self::Line { a, b } => vec![a, b],
            Self::Plant { centre, front, .. }
            | Self::Water { centre, front, .. }
            | Self::Station { centre, front } => vec![centre, front],
        }
    }
}

/// What a step did, for the buttons' labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    Road,
    Curve,
    Grid,
    Class,
    Bulldoze,
    Plant(PlantKind),
    Line,
    Water(WaterKind),
    Station,
}

impl StepKind {
    /// The kind of step a command makes, or None for commands outside the history.
    fn of(cmd: &Command) -> Option<Self> {
        Some(match *cmd {
            Command::BuildRoad { .. } => Self::Road,
            Command::BuildCurve { .. } => Self::Curve,
            Command::BuildGrid { .. } => Self::Grid,
            Command::SetRoadClass { .. } => Self::Class,
            Command::DeleteRoad { .. } => Self::Bulldoze,
            Command::PlacePlant { kind, .. } => Self::Plant(kind),
            Command::BuildLine { .. } => Self::Line,
            Command::PlaceWater { kind, .. } => Self::Water(kind),
            Command::PlaceStation { .. } => Self::Station,
            _ => return None,
        })
    }
}

/// One undoable edit. Undo removes `after` and restores `before`, refunding `cost`; redo
/// removes `before` and restores `after`, charging it again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    pub before: Vec<Record>,
    pub after: Vec<Record>,
    /// Whole dollars the edit took from the balance.
    pub cost: i64,
}

/// The undo and redo lists, newest last, saved with the city.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// Whether the newest undo step is a continuous curve chain that later pieces join.
    chain_open: bool,
}

impl History {
    /// The step `Undo` would take back.
    pub fn undo_step(&self) -> Option<&Step> {
        self.undo.last()
    }

    /// The step `Redo` would make again.
    pub fn redo_step(&self) -> Option<&Step> {
        self.redo.last()
    }

    pub fn len(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }

    /// Drops the oldest undo steps past the caps; the newest step always stays.
    fn trim(&mut self) {
        let records = |s: &Step| s.before.len() + s.after.len();
        let mut total: usize = self.undo.iter().chain(&self.redo).map(records).sum();
        let mut drop = 0;
        while self.undo.len() - drop > 1
            && (self.undo.len() - drop + self.redo.len() > MAX_STEPS || total > MAX_RECORDS)
        {
            total -= records(&self.undo[drop]);
            drop += 1;
        }
        self.undo.drain(..drop);
    }

    pub(crate) fn check(&self) -> Check {
        let steps = || self.undo.iter().chain(&self.redo);
        let records: usize = steps().map(|s| s.before.len() + s.after.len()).sum();
        ensure(
            self.undo.len() + self.redo.len() <= MAX_STEPS && records <= MAX_RECORDS,
            "history size",
        )?;
        ensure(
            steps().all(|s| s.cost >= 0)
                && steps()
                    .flat_map(|s| s.before.iter().chain(&s.after))
                    .flat_map(Record::points)
                    .all(|p| {
                        p.0.is_finite() && p.1.is_finite() && p.0.abs() <= 1e6 && p.1.abs() <= 1e6
                    }),
            "history records",
        )
    }
}

impl World {
    pub fn history(&self) -> &History {
        &self.history
    }

    /// What a history command needs captured before it runs, or None for other commands.
    pub(super) fn before_command(&self, cmd: &Command) -> Option<(StepKind, Capture)> {
        StepKind::of(cmd).map(|kind| (kind, Capture::of(self)))
    }

    /// Records a successful command. A history command that changed placeables makes a step,
    /// or joins the open curve chain's step, and clears the redo list. Every other success
    /// closes the chain.
    pub(super) fn after_command(&mut self, chain: bool, captured: Option<(StepKind, Capture)>) {
        let Some((kind, capture)) = captured else {
            self.history.chain_open = false;
            return;
        };
        let seen = capture.diff(self);
        if seen.before.is_empty() && seen.after.is_empty() {
            self.history.chain_open = false;
            return;
        }
        let h = &mut self.history;
        match h.undo.last_mut() {
            Some(step) if chain && h.chain_open => {
                for r in seen.before {
                    match step.after.iter().position(|&a| a == r) {
                        Some(i) => drop(step.after.remove(i)),
                        None => step.before.push(r),
                    }
                }
                step.after.extend(seen.after);
                step.cost += seen.cost;
            }
            _ => h.undo.push(Step {
                kind,
                before: seen.before,
                after: seen.after,
                cost: seen.cost,
            }),
        }
        h.chain_open = kind == StepKind::Curve;
        h.redo.clear();
        h.trim();
    }
}
