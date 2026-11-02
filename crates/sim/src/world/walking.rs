//! Citizens on foot (ADR 0026): walk routes between building fronts, and walkers stepping
//! along sidewalks each tick. Walkers stay out of lane queues, so they never block cars or
//! each other. They wait only at red signals.

use super::World;
use crate::agents::{AgentId, AgentState};
use crate::building::BuildingId;
use crate::citizen::CitizenId;
use crate::road::{LaneId, RoadId};

impl World {
    /// Straight-line distance between two buildings' front points, metres.
    pub(super) fn front_distance(&self, a: BuildingId, b: BuildingId) -> f32 {
        let (la, lb) = (
            self.buildings.lot[a as usize] as usize,
            self.buildings.lot[b as usize] as usize,
        );
        (self.lots.front_x[la] - self.lots.front_x[lb])
            .hypot(self.lots.front_y[la] - self.lots.front_y[lb])
    }

    /// How far citizen `c` may walk for a job (`job`) or their next stop, or `None` if they
    /// can drive there. Jobs are judged from home; other stops from where `c` is now, where a
    /// car is only at hand if it came along (ADR 0026).
    pub(super) fn walk_reach(&self, c: CitizenId, job: bool) -> Option<f32> {
        let i = c as usize;
        let drives = if job || self.citizens.location[i] == self.home_building(c) {
            self.citizens.owns_car[i]
        } else {
            self.citizens.car_out[i]
        };
        (!drives).then_some(self.citizens.walk_limit[i])
    }

    /// The road building `b` fronts and its front point's distance along that road's lane
    /// `2k`, or `None` without access.
    fn frontage(&self, b: BuildingId) -> Option<(RoadId, f32)> {
        self.building_node(b)?;
        let l = self.buildings.lot[b as usize] as usize;
        let road = self.lots.road[l];
        let front = (self.lots.front_x[l], self.lots.front_y[l]);
        Some((road, self.roads.road_offset(road, front)))
    }

    /// Distance along `lane` of the point `offset` metres along its road's lane `2k`.
    fn along(&self, lane: LaneId, offset: f32) -> f32 {
        if lane.is_multiple_of(2) {
            offset
        } else {
            self.roads.lane_len[lane as usize] - offset
        }
    }

    /// Puts citizen `c`'s slot `agent` on foot from their location's front point to their
    /// destination's, over the shortest walk. False if either end lacks access or no route
    /// links them.
    pub(super) fn plan_walk(&mut self, agent: AgentId, c: CitizenId) -> bool {
        let i = c as usize;
        let (origin, Some(dest)) = (self.citizens.location[i], self.citizens.destination[i]) else {
            return false;
        };
        let (Some((from_road, from_at)), Some((to_road, to_at))) =
            (self.frontage(origin), self.frontage(dest))
        else {
            return false;
        };
        let from = self.building_node(origin).expect("frontage implies access");
        let to = self.building_node(dest).expect("frontage implies access");
        let a = agent as usize;
        let mut path = std::mem::take(&mut self.agents.path[a]);
        path.clear();
        let (start, end) = if from_road == to_road {
            let lane = from_road * 2 + u32::from(to_at < from_at);
            path.push(lane);
            (self.along(lane, from_at), self.along(lane, to_at))
        } else {
            let mut route = std::mem::take(&mut self.detour);
            let found = self.pathfinder.find_walk(&self.roads, from, to, &mut route);
            self.totals.path_nodes_expanded += u64::from(self.pathfinder.last_expanded);
            if !found {
                self.detour = route;
                self.agents.path[a] = path;
                return false;
            }
            // Walk to the origin road's end at `from`, unless the route turns straight back
            // along the origin road; then start on that lane. Likewise at the far end.
            let mut first = route.len().min(1);
            let lane = if first == 1 && route[0] / 2 == from_road {
                route[0]
            } else {
                first = 0;
                from_road * 2 + u32::from(self.roads.lane_to[from_road as usize * 2] != from)
            };
            path.push(lane);
            let start = self.along(lane, from_at);
            let mut last = route.len();
            let lane = if last > first && route[last - 1] / 2 == to_road {
                last -= 1;
                route[last]
            } else {
                to_road * 2 + u32::from(self.roads.lane_from[to_road as usize * 2] != to)
            };
            path.extend_from_slice(&route[first..last]);
            path.push(lane);
            self.detour = route;
            (start, self.along(lane, to_at))
        };
        let agents = &mut self.agents;
        agents.lane[a] = path[0];
        agents.path[a] = path;
        agents.path_idx[a] = 0;
        agents.pos[a] = start;
        agents.walk_end[a] = end;
        agents.speed[a] = self.citizens.walk_speed[i];
        agents.node[a] = from;
        agents.dest[a] = to;
        agents.stuck_ticks[a] = 0;
        agents.cleared[a] = false;
        agents.state[a] = AgentState::Walking;
        true
    }

    /// Moves every walker one tick along its sidewalks and ends finished walks.
    pub(super) fn advance_walkers(&mut self) {
        for k in 0..self.walking.len() {
            let agent = self.walking[k];
            if self.step_walker(agent) {
                let c = self.agents.citizen[agent as usize].expect("walkers are citizens");
                self.totals.trips_completed += 1;
                self.totals.walks_completed += 1;
                self.citizen_arrived(c);
            }
        }
        let state = &self.agents.state;
        self.walking
            .retain(|&agent| state[agent as usize] == AgentState::Walking);
    }

    /// One tick of walking; true when `agent` reaches the end of its walk. A walker reaching
    /// a red signal waits at the stop line and calls for a green.
    fn step_walker(&mut self, agent: AgentId) -> bool {
        let a = agent as usize;
        let c = self.agents.citizen[a].expect("walkers are citizens") as usize;
        let speed = self.citizens.walk_speed[c];
        let mut before = self.agents.pos[a];
        let mut pos = before + speed * self.cfg.dt;
        let mut moving = true;
        loop {
            let lane = self.agents.lane[a];
            let next = self.agents.path_idx[a] as usize + 1;
            if next == self.agents.path[a].len() {
                if pos >= self.agents.walk_end[a] {
                    return true;
                }
                break;
            }
            let stop = self.junctions.stop[lane as usize];
            if before <= stop && pos > stop && !self.junctions.walk_allowed(&self.roads, lane) {
                self.junctions.call_walk(&self.roads, lane);
                pos = stop;
                moving = false;
                break;
            }
            let len = self.roads.lane_len[lane as usize];
            if pos < len {
                break;
            }
            pos -= len;
            before = 0.0;
            self.agents.path_idx[a] = next as u32;
            self.agents.lane[a] = self.agents.path[a][next];
        }
        self.agents.pos[a] = pos;
        self.agents.speed[a] = if moving { speed } else { 0.0 };
        false
    }
}
