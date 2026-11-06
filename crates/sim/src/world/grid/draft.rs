//! The grid tool's world side: the command from raw input, previews and the build.

use super::{GridPlan, Lattice, MAX_BLOCKS, plan};
use crate::geometry::dist;
use crate::road::RoadClass;
use crate::world::{Command, World};

/// What a `BuildGrid` would build, for tests and tools. Changes nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct GridPreview {
    /// `[ax, ay, a_join, bx, by, b_join]` per new leg; join 0 = new end, 1 = existing node,
    /// 2 = splits a road.
    pub roads: Vec<[f32; 6]>,
    /// `[ax, ay, bx, by]` per lattice edge left out.
    pub dropped: Vec<[f32; 4]>,
    /// Dollars, None when nothing would be built.
    pub cost: Option<i64>,
}

/// The lattice and class of a `BuildGrid` the sim accepts; None for any other command, an
/// alley grid or numbers out of range.
fn lattice_of(cmd: &Command) -> Option<(Lattice, RoadClass)> {
    let &Command::BuildGrid {
        ox,
        oy,
        ux,
        uy,
        cell,
        cols,
        rows,
        class,
    } = cmd
    else {
        return None;
    };
    let lattice = Lattice {
        origin: (ox, oy),
        axis: (ux, uy),
        cell,
        cols,
        rows,
    };
    (lattice.valid() && class != RoadClass::Alley).then_some((lattice, class))
}

impl World {
    /// The `BuildGrid` for the first edge's two raw `[x, y, snap]` points, guided as a road's
    /// (ADR 0038), and the area `cursor`, rounded to whole blocks of `cell` metres, at least
    /// one and at most `MAX_BLOCKS` each way. Without a cursor, the edge's end stands in.
    /// None if the points are not an edge or `cell` is out of range.
    pub fn grid_command(
        &self,
        raw: &[f32],
        cursor: Option<(f32, f32)>,
        cell: f32,
        class: RoadClass,
    ) -> Option<Command> {
        let [o, e] = self.draft(raw).points[..] else {
            return None;
        };
        let len = dist(o, e);
        if !len.is_finite() || len < 1e-3 || !(super::MIN_CELL..=super::MAX_CELL).contains(&cell) {
            return None;
        }
        let (ux, uy) = ((e.0 - o.0) / len, (e.1 - o.1) / len);
        let c = cursor.unwrap_or(e);
        let w = (c.0 - o.0, c.1 - o.1);
        let blocks = |along: f32| {
            let n = (along / cell)
                .round()
                .clamp(-MAX_BLOCKS as f32, MAX_BLOCKS as f32) as i32;
            match n {
                0 if along < 0.0 => -1,
                0 => 1,
                n => n,
            }
        };
        if !w.0.is_finite() || !w.1.is_finite() {
            return None;
        }
        Some(Command::BuildGrid {
            ox: o.0,
            oy: o.1,
            ux,
            uy,
            cell,
            cols: blocks(w.0 * ux + w.1 * uy),
            rows: blocks(w.1 * ux - w.0 * uy),
            class,
        })
    }

    /// What `cmd`, a `BuildGrid`, would build (ADR 0049); None if it would be refused for its
    /// numbers or class (not for money). Changes nothing.
    pub fn grid_preview(&self, cmd: &Command) -> Option<GridPreview> {
        let (lattice, class) = lattice_of(cmd)?;
        let p = plan(&self.roads, &lattice, class);
        Some(GridPreview {
            cost: (!p.plans.is_empty()).then_some(p.cost),
            roads: p.roads,
            dropped: p.dropped,
        })
    }

    /// Guides the first edge and writes `[n, m, length, angle, k, cols, rows, cost, r]`, then
    /// `[x, y]` per guided point, `[x0, y0, x1, y1]` per guide line, `[x, y, rx, ry, dx, dy]`
    /// per angle mark (the edge's start, then its end), `[ax, ay, a_join, bx, by, b_join]` per
    /// new leg and `[ax, ay, bx, by]` per left-out lattice edge to the end. Cols and rows are 0
    /// and cost NaN when nothing is planned. Changes nothing.
    pub fn write_grid_draft(
        &self,
        raw: &[f32],
        cursor: Option<(f32, f32)>,
        cell: f32,
        class: RoadClass,
        out: &mut Vec<f32>,
    ) {
        out.clear();
        let d = self.draft(raw);
        let marks: Vec<_> = d.start_mark.into_iter().chain(d.end_mark).collect();
        let cmd = self.grid_command(raw, cursor, cell, class);
        let (cols, rows) = match cmd {
            Some(Command::BuildGrid { cols, rows, .. }) => (cols, rows),
            _ => (0, 0),
        };
        let preview = cmd.and_then(|c| self.grid_preview(&c));
        let (roads, dropped, cost) = match preview {
            Some(p) => (p.roads, p.dropped, p.cost),
            None => (Vec::new(), Vec::new(), None),
        };
        out.extend([
            d.points.len() as f32,
            d.lines.len() as f32,
            d.length,
            d.angle,
            marks.len() as f32,
            cols as f32,
            rows as f32,
            cost.map_or(f32::NAN, |c| c as f32),
            roads.len() as f32,
        ]);
        out.extend(d.points.iter().flat_map(|p| [p.0, p.1]));
        out.extend(d.lines.iter().flatten());
        out.extend(marks.iter().flatten());
        out.extend(roads.iter().flatten());
        out.extend(dropped.iter().flatten());
    }

    /// Builds `cmd`, a `BuildGrid`, all or nothing: refused for its numbers or class, when
    /// nothing would be built, or when the city cannot pay for all of it (ADR 0036, 0049).
    pub(in crate::world) fn build_grid(&mut self, cmd: &Command) -> bool {
        let Some((lattice, class)) = lattice_of(cmd) else {
            return false;
        };
        let GridPlan { plans, cost, .. } = plan(&self.roads, &lattice, class);
        self.build_plans(plans, cost, class)
    }
}
