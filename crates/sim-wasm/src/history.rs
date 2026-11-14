//! Undo and redo (ADR 0051): type conversion only, like the rest of this crate.

use sim::Command;
use sim::power::PlantKind;
use sim::water::WaterKind;
use sim::world::{SchoolKind, StepKind};
use wasm_bindgen::prelude::*;

use crate::Sim;

/// What the undo and redo buttons show; see `sim::HistoryView`. Kinds are names the frontend
/// labels: road, curve, grid, class, bulldoze, coal, solar, wind, line, pump, tower, station, primary, high.
#[wasm_bindgen(getter_with_clone)]
pub struct HistoryView {
    pub undo: Option<String>,
    pub redo: Option<String>,
    pub undo_refund: Option<f64>,
    pub redo_cost: Option<f64>,
    pub redo_affordable: bool,
}

fn name(kind: StepKind) -> String {
    match kind {
        StepKind::Road => "road",
        StepKind::Curve => "curve",
        StepKind::Grid => "grid",
        StepKind::Class => "class",
        StepKind::Bulldoze => "bulldoze",
        StepKind::Plant(PlantKind::Coal) => "coal",
        StepKind::Plant(PlantKind::Solar) => "solar",
        StepKind::Plant(PlantKind::Wind) => "wind",
        StepKind::Line => "line",
        StepKind::Water(WaterKind::Pump) => "pump",
        StepKind::Water(WaterKind::Tower) => "tower",
        StepKind::Station => "station",
        StepKind::School(SchoolKind::Primary) => "primary",
        StepKind::School(SchoolKind::High) => "high",
    }
    .to_owned()
}

#[wasm_bindgen]
impl Sim {
    /// Takes back the newest undoable edit; false if there is none or it no longer fits.
    pub fn undo(&mut self) -> bool {
        self.apply(Command::Undo)
    }

    /// Makes the newest undone edit again; false if there is none, it no longer fits or the
    /// city cannot pay.
    pub fn redo(&mut self) -> bool {
        self.apply(Command::Redo)
    }

    pub fn history_view(&self) -> HistoryView {
        let v = self.world.history_view();
        HistoryView {
            undo: v.undo.map(name),
            redo: v.redo.map(name),
            undo_refund: v.undo_refund.map(|d| d as f64),
            redo_cost: v.redo_cost.map(|d| d as f64),
            redo_affordable: v.redo_affordable,
        }
    }

    /// `[ok, remove_floats, remove…, restore…]`: whether undo (or `redo`) would succeed, how
    /// many floats of removed records follow, then the removed and restored records, each
    /// `[n, x0, y0, …]`; see `World::write_history_preview`.
    pub fn history_preview(&mut self, redo: bool) -> Vec<f32> {
        let mut restore = Vec::new();
        let ok = self
            .world
            .write_history_preview(redo, &mut self.buf, &mut restore);
        let mut out = Vec::with_capacity(2 + self.buf.len() + restore.len());
        out.extend([f32::from(u8::from(ok)), self.buf.len() as f32]);
        out.extend_from_slice(&self.buf);
        out.extend_from_slice(&restore);
        out
    }
}
