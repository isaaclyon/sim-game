//! Road editing: building, splitting and bulldozing roads.

use super::World;
use super::zoning::Scope;
use crate::block::{self, Rect};
use crate::geometry::Shape;
use crate::road::{NodeId, RoadClass, RoadId};
use crate::snap::{self, Join};

impl World {
    /// Builds a road of `class` along `snap::plan`: joining nodes near its ends, and splitting
    /// the roads it crosses or ends beside so they meet at a junction.
    pub(super) fn build_road(&mut self, shape: Shape, class: RoadClass) -> bool {
        let mut split = Vec::new();
        let Some(rect) = self.add_road(shape, class, &mut split) else {
            return false;
        };
        self.sync_roads();
        self.remove_cars_using(&split);
        self.update_lots(Scope::Near(rect), true);
        true
    }

    /// Adds the roads `snap::plan` makes of `shape`, pushing the roads it splits onto `split`.
    /// Returns the area it changed. The caller syncs the network and removes affected cars.
    pub(super) fn add_road(
        &mut self,
        shape: Shape,
        class: RoadClass,
        split: &mut Vec<RoadId>,
    ) -> Option<Rect> {
        let plan = snap::plan(&self.roads, shape)?;
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
            // False where the plan follows an existing road, which is kept as it is.
            self.roads.add_shape(leg[0], leg[1], shape.control, class);
        }
        Some(rect)
    }

    pub(super) fn delete_road(&mut self, x: f32, y: f32) -> bool {
        let Some(road) = snap::bulldoze_target(&self.roads, x, y) else {
            return false;
        };
        self.roads.remove_road(road);
        self.remove_cars_using(&[road]);
        self.sync_roads();
        self.update_lots(Scope::Near(block::shape_rect(self.roads.shape(road))), true);
        true
    }
}
