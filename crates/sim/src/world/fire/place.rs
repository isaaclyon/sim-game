//! Placing and bulldozing fire stations, and clearing ruins early (ADR 0053).

use super::{STATION_COST, STATION_HALF, TRUCKS_PER_STATION, Task};
use crate::agents::AgentState;
use crate::frontage::frontage;
use crate::power::{COAL_SETBACK, COAL_SNAP, Footprint, PlantKind};
use crate::world::World;

impl World {
    /// Where a station asked for at the cursor would stand: beside the nearest road in reach,
    /// facing it, as a coal plant does (ADR 0042). `None` off the map or away from roads.
    pub fn station_spot(&self, x: f32, y: f32) -> Option<Footprint> {
        if !(x.is_finite() && y.is_finite() && x.abs() < 1e6 && y.abs() < 1e6) {
            return None;
        }
        let f = frontage(&self.roads, (x, y), COAL_SNAP)?;
        Some(Footprint {
            centre: f.out(f.kerb + COAL_SETBACK + STATION_HALF),
            front: f.front,
            half: STATION_HALF,
        })
    }

    /// Whether a station fits at its `station_spot`: beside a road and clear of roads, plants
    /// and stations. Says nothing about money.
    pub fn station_fits(&self, x: f32, y: f32) -> bool {
        self.station_spot(x, y)
            .is_some_and(|at| self.spot_fits(PlantKind::Coal, at))
    }

    /// Places a station and its trucks, clearing the land under it. Refused where it does not
    /// fit or when the city cannot pay.
    pub(in crate::world) fn place_station(&mut self, x: f32, y: f32) -> bool {
        let Some(at) = self.station_spot(x, y) else {
            return false;
        };
        if !self.spot_fits(PlantKind::Coal, at) || !self.can_afford(STATION_COST) {
            return false;
        }
        self.treasury.balance -= STATION_COST;
        let f = &mut self.fire;
        let s = f.station_active.len() as u32;
        f.station_x.push(at.centre.0);
        f.station_y.push(at.centre.1);
        f.station_fx.push(at.front.0);
        f.station_fy.push(at.front.1);
        f.station_active.push(true);
        f.bump();
        self.ensure_cover();
        let home = self.fire.cover.node[s as usize].unwrap_or(0);
        for _ in 0..TRUCKS_PER_STATION {
            let agent = self.agents.push(home);
            self.agents.state[agent as usize] = AgentState::Dormant;
            let f = &mut self.fire;
            f.truck_agent.push(agent);
            f.truck_station.push(s);
            f.truck_task.push(Task::Home);
            f.mark_truck(agent);
        }
        self.clear_footprint(at);
        self.refresh_upkeep();
        true
    }

    /// Bulldozes the station under the point; its trucks are taken off the roads for good.
    pub(in crate::world) fn bulldoze_station(&mut self, x: f32, y: f32) -> bool {
        let Some(s) = self.station_at(x, y) else {
            return false;
        };
        self.fire.station_active[s as usize] = false;
        for t in 0..self.fire.truck_agent.len() {
            if self.fire.truck_station[t] == s {
                self.unassign(t);
                self.take_off_road(t);
                self.fire.truck_task[t] = Task::Retired;
            }
        }
        self.fire.bump();
        self.refresh_upkeep();
        true
    }

    /// The active station whose footprint holds the point.
    pub(in crate::world) fn station_at(&self, x: f32, y: f32) -> Option<u32> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        self.fire
            .active_stations()
            .find(|&s| self.fire.footprint(s).contains((x, y), 0.0))
    }

    /// Whether a station overlaps the footprint, so nothing else is placed there.
    pub(in crate::world) fn station_overlaps(&self, at: Footprint) -> bool {
        self.fire
            .active_stations()
            .any(|s| self.fire.footprint(s).overlaps(at))
    }

    /// Whether a station stands on the lot, so nothing grows there.
    pub(in crate::world) fn station_blocks(&self, lot: crate::lot::LotId) -> bool {
        let ring = self.lots.outline(lot);
        self.fire
            .active_stations()
            .any(|s| self.fire.footprint(s).hits_ring(ring))
    }

    /// The ruins whose lot holds the point.
    pub(in crate::world) fn ruin_at(&self, x: f32, y: f32) -> Option<usize> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        self.fire
            .ruin_lot
            .iter()
            .position(|&l| crate::poly::contains(self.lots.outline(l), (x, y)))
    }

    /// Clears the ruins under the point, so the lot may grow again at once.
    pub(in crate::world) fn clear_ruin(&mut self, x: f32, y: f32) -> bool {
        let Some(r) = self.ruin_at(x, y) else {
            return false;
        };
        self.fire.ruin_lot.remove(r);
        self.fire.ruin_until.remove(r);
        self.fire.bump();
        true
    }
}
