//! Consistency checks for a loaded network (ADR 0033).

use std::collections::BTreeSet;

use super::{RoadId, RoadNetwork};

impl RoadNetwork {
    /// Internal consistency of a loaded network.
    pub(crate) fn check(&self) -> crate::check::Check {
        use crate::check::{below, csr, ensure, finite, lens};
        let (nodes, lanes, roads) = (self.node_count(), self.lane_count(), self.road_active.len());
        ensure(self.node_y.len() == nodes, "node coordinates")?;
        lens(
            &[
                self.lane_to.len(),
                self.lane_len.len(),
                self.lane_speed.len(),
            ],
            lanes,
            "lanes",
        )?;
        lens(
            &[self.class.len(), self.control.len(), self.lengths.len()],
            roads,
            "roads",
        )?;
        ensure(lanes == 2 * roads, "two lanes per road")?;
        finite(self.node_x.iter().chain(&self.node_y), "node positions")?;
        ensure(
            self.lane_len.iter().all(|l| l.is_finite() && *l >= 0.0)
                && self.lane_speed.iter().all(|v| v.is_finite() && *v > 0.0),
            "lane lengths and speeds",
        )?;
        let curves = self.control.iter().flatten().flat_map(|p| [&p.0, &p.1]);
        finite(
            curves.chain(self.lengths.iter().flatten()),
            "curve geometry",
        )?;
        below(
            self.lane_from.iter().chain(&self.lane_to),
            nodes,
            "lane ends",
        )?;
        ensure(
            self.control
                .iter()
                .zip(&self.lengths)
                .all(|(c, l)| c.is_some() != l.is_empty()),
            "curve lengths",
        )?;
        let ends = |r: RoadId| {
            let (a, b) = self.road_ends(r);
            (a.min(b), a.max(b))
        };
        let active: BTreeSet<_> = self.active_roads().map(ends).collect();
        ensure(
            active.len() == self.active_roads().count() && active == self.roads,
            "road index",
        )?;
        if !self.adjacency_dirty {
            csr(&self.out_start, nodes, self.out_lanes.len(), "adjacency")?;
            below(&self.out_lanes, lanes, "adjacency lanes")?;
        }
        Ok(())
    }
}
