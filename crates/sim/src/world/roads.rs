//! Road editing: building, splitting and bulldozing roads.

use super::zoning::Scope;
use super::{Command, World, money};
use crate::block::{self, Rect};
use crate::geometry::Shape;
use crate::guide;
use crate::road::{NodeId, RoadClass, RoadId};
use crate::snap::{self, Join};

impl World {
    /// Guides raw `[x, y, snap]` points (ADR 0038). Extra trailing floats are ignored.
    pub(super) fn draft(&self, raw: &[f32]) -> guide::Draft {
        let points: Vec<_> = raw
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| (p[0], p[1], p[2] as u8))
            .collect();
        guide::draft(&self.roads, &points)
    }

    /// The command that builds the road through guided raw points: two for a straight road,
    /// three for a curve (start, control, end). None for any other count.
    pub fn draft_command(&self, raw: &[f32], class: RoadClass) -> Option<Command> {
        match self.draft(raw).points[..] {
            [(ax, ay), (bx, by)] => Some(Command::BuildRoad {
                ax,
                ay,
                bx,
                by,
                class,
            }),
            [(ax, ay), (cx, cy), (bx, by)] => Some(Command::BuildCurve {
                ax,
                ay,
                cx,
                cy,
                bx,
                by,
                class,
            }),
            _ => None,
        }
    }

    /// Builds a road of `class` along `snap::plan`: joining nodes near its ends, splitting
    /// the roads it crosses or ends beside so they meet at a junction, and giving the roads it
    /// follows its class (ADR 0048). Refused when the city cannot afford it (ADR 0036).
    pub(super) fn build_road(&mut self, shape: Shape, class: RoadClass) -> bool {
        let Some(plan) = snap::plan(&self.roads, shape, Some(class)) else {
            return false;
        };
        let cost = money::plan_cost(&self.roads, &plan, class);
        if !self.can_afford(cost) {
            return false;
        }
        self.treasury.balance -= cost;
        let mut split = Vec::new();
        let rect = self.add_plan(plan, class, &mut split);
        split.extend(self.join_straight_roads());
        self.sync_roads();
        self.remove_cars_using(&split);
        self.update_lots(Scope::Near(rect), true);
        true
    }

    /// Adds the roads `snap::plan` makes of `shape`, pushing the roads it splits onto `split`.
    /// Roads it follows keep their class. Returns the area it changed. The caller syncs the
    /// network and removes affected cars.
    pub(super) fn add_road(
        &mut self,
        shape: Shape,
        class: RoadClass,
        split: &mut Vec<RoadId>,
    ) -> Option<Rect> {
        let plan = snap::plan(&self.roads, shape, None)?;
        Some(self.add_plan(plan, class, split))
    }

    fn add_plan(&mut self, plan: snap::Plan, class: RoadClass, split: &mut Vec<RoadId>) -> Rect {
        let nodes: Vec<NodeId> = plan
            .stops
            .iter()
            .map(|s| match s.join {
                Join::Node(n) => n,
                Join::New => self.roads.add_node(s.x, s.y),
                Join::Split(road) => {
                    // The plan splits each road at most once, so `road` still has its old ends.
                    let n = self.roads.add_node(s.x, s.y);
                    let rest = self.roads.split_road(road, n);
                    self.junctions.remap_split(road, rest);
                    split.push(road);
                    n
                }
            })
            .collect();
        let mut rect = block::shape_rect(plan.legs[0]);
        for (leg, shape) in nodes.windows(2).zip(plan.legs) {
            rect = block::union(rect, block::shape_rect(shape));
            // False where the plan follows an existing road, which is kept in place.
            self.roads.add_shape(leg[0], leg[1], shape.control, class);
        }
        // Lanes and shapes stay, so cars on these roads keep their routes.
        for road in plan.reclass {
            self.roads.set_class(road, class);
        }
        rect
    }

    pub(super) fn delete_road(&mut self, x: f32, y: f32) -> bool {
        let Some(road) = snap::bulldoze_target(&self.roads, x, y) else {
            return false;
        };
        self.roads.remove_road(road);
        let mut changed = self.join_straight_roads();
        changed.push(road);
        self.remove_cars_using(&changed);
        self.sync_roads();
        self.update_lots(Scope::Near(block::shape_rect(self.roads.shape(road))), true);
        true
    }

    /// Joins every pair of straight roads meeting end to end in a line (ADR 0040). Their lots
    /// move to the kept road. Returns the roads changed, whose cars the caller removes.
    fn join_straight_roads(&mut self) -> Vec<RoadId> {
        let mut changed = Vec::new();
        for n in self.roads.two_road_nodes() {
            let Some(j) = self.roads.join_straight(n) else {
                continue;
            };
            self.junctions.remap_lanes(&j.lanes);
            for road in &mut self.lots.road {
                if *road == j.removed {
                    *road = j.kept;
                }
            }
            changed.extend([j.kept, j.removed]);
        }
        changed
    }
}
