//! Road drafting, grids and class changes: guided previews, quotes and builds.

use sim::Command;
use sim::road::RoadClass;
use wasm_bindgen::prelude::*;

use crate::Sim;

#[wasm_bindgen]
impl Sim {
    /// Guides raw `[x, y, snap]` points (then optionally one float, the snap reach in metres;
    /// `guide::draft_within`) and, if `plan`, previews the road of `class` through
    /// them; see `World::write_road_draft` (ADR 0038, 0048). Unknown classes plan nothing.
    pub fn road_draft(&mut self, points: &[f32], plan: bool, class: u8) -> Vec<f32> {
        let (plan, class) = match RoadClass::from_u8(class) {
            Some(class) => (plan, class),
            None => (false, RoadClass::Street),
        };
        self.world
            .write_road_draft(points, plan, class, &mut self.buf);
        self.buf.clone()
    }

    /// Builds the road through guided raw `[x, y, snap]` points (two, or three for a curve),
    /// optionally followed by the snap reach, as in `road_draft`.
    /// Returns the guided end `[x, y]`, or nothing if refused. `class` is 0 = street,
    /// 1 = main road, 2 = alley; unknown values build nothing.
    pub fn build_draft(&mut self, points: &[f32], class: u8) -> Vec<f32> {
        let Some(class) = RoadClass::from_u8(class) else {
            return Vec::new();
        };
        let Some(cmd) = self.world.draft_command(points, class) else {
            return Vec::new();
        };
        let end = match cmd {
            Command::BuildRoad { bx, by, .. } | Command::BuildCurve { bx, by, .. } => vec![bx, by],
            _ => Vec::new(),
        };
        if self.apply(cmd) { end } else { Vec::new() }
    }

    /// What `build_draft` would charge in dollars (ADR 0036), or undefined if it would be
    /// refused for its shape. Unknown classes cost nothing to quote and build nothing.
    pub fn draft_cost(&self, points: &[f32], class: u8) -> Option<f64> {
        let class = RoadClass::from_u8(class)?;
        Some(self.world.draft_cost(points, class)? as f64)
    }

    /// Whether the city can pay `cost` dollars now; see `World::can_afford`.
    pub fn can_afford(&self, cost: f64) -> bool {
        self.world.can_afford(cost as i64)
    }

    /// Gives the road nearest the point `class` in place (ADR 0050). Unknown classes change
    /// nothing.
    pub fn set_road_class(&mut self, x: f32, y: f32, class: u8) -> bool {
        let Some(class) = RoadClass::from_u8(class) else {
            return false;
        };
        self.apply(Command::SetRoadClass { x, y, class })
    }

    /// `[x, y]` per centreline sample of the road `set_road_class` would change, or empty.
    pub fn class_preview(&mut self, x: f32, y: f32, class: u8) -> Vec<f32> {
        match RoadClass::from_u8(class) {
            Some(class) => self.world.write_class_preview(x, y, class, &mut self.buf),
            None => self.buf.clear(),
        }
        self.buf.clone()
    }

    /// What `set_road_class` would charge in dollars, or undefined if it would change nothing.
    pub fn class_cost(&self, x: f32, y: f32, class: u8) -> Option<f64> {
        let class = RoadClass::from_u8(class)?;
        Some(self.world.class_cost(x, y, class)? as f64)
    }

    /// Guides a grid's first edge, two raw `[x, y, snap]` points and an optional snap reach as
    /// in `road_draft`, and previews the grid of
    /// `cell`-metre blocks reaching `cursor` (`[x, y]`, or empty while the edge is drawn); see
    /// `World::write_grid_draft` (ADR 0049). Unknown classes plan nothing.
    pub fn grid_draft(&mut self, points: &[f32], cursor: &[f32], cell: f32, class: u8) -> Vec<f32> {
        let class = RoadClass::from_u8(class).unwrap_or(RoadClass::Alley);
        self.world
            .write_grid_draft(points, cursor_of(cursor), cell, class, &mut self.buf);
        self.buf.clone()
    }

    /// Builds the grid `grid_draft` previews; false if refused. Unknown classes build nothing.
    pub fn build_grid_draft(
        &mut self,
        points: &[f32],
        cursor: &[f32],
        cell: f32,
        class: u8,
    ) -> bool {
        let Some(class) = RoadClass::from_u8(class) else {
            return false;
        };
        let cmd = self
            .world
            .grid_command(points, cursor_of(cursor), cell, class);
        cmd.is_some_and(|cmd| self.apply(cmd))
    }
}

fn cursor_of(cursor: &[f32]) -> Option<(f32, f32)> {
    match *cursor {
        [x, y] => Some((x, y)),
        _ => None,
    }
}
