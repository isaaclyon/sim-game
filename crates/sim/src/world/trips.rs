//! Agent trips: spawning, parking, routing, lane entry, junction exits and detours.

use std::cmp::Reverse;

use super::World;
use crate::agents::{AgentId, AgentState};
use crate::junction::Verdict;
use crate::road::{LaneId, RoadId};
use crate::traffic::Traffic;

/// A car held this long by a full exit looks for another way, then retries this often, seconds.
const DETOUR_AFTER_S: f32 = 10.0;
const DETOUR_EVERY_S: f32 = 5.0;
/// A detour search gives up after expanding this many nodes.
const DETOUR_MAX_NODES: u32 = 256;
/// Wait before retrying after a failed path or a stuck despawn, seconds.
const RETRY_AFTER_S: f32 = 10.0;
/// Separates the route-taste hash from other hashes of the world seed.
const ROUTE_TASTE_SALT: u64 = 0x726f_7574;

impl World {
    pub(super) fn spawn_agents(&mut self, count: u32) -> bool {
        self.sync_roads();
        let starts: Vec<_> = (0..self.roads.node_count() as u32)
            .filter(|&n| !self.roads.out_lanes(n).is_empty())
            .collect();
        if starts.len() < 2 {
            return false;
        }
        for _ in 0..count {
            let node = starts[self.rng.below(starts.len() as u32) as usize];
            let agent = self.agents.push(node);
            // Stagger first departures so traffic ramps up instead of arriving in one wave.
            let delay = self.rng.range(0.0, self.cfg.dwell_max_s);
            self.park(agent, delay);
        }
        true
    }

    /// Removes cars whose remaining route uses a lane of `roads`, just split or deleted.
    /// They wait and start a new trip, like a stuck despawn. Junction reservations are left
    /// to expire; both splits and tombstones keep the headings those reservations use.
    pub(super) fn remove_cars_using(&mut self, roads: &[RoadId]) {
        if roads.is_empty() {
            return;
        }
        let uses = |lanes: &[LaneId]| lanes.iter().any(|l| roads.contains(&(l / 2)));
        for agent in 0..self.agents.len() as AgentId {
            let a = agent as usize;
            let state = self.agents.state[a];
            // Only cars with a current route; a parked car's old route is stale.
            if !matches!(state, AgentState::Driving | AgentState::Entering)
                || !uses(&self.agents.path[a][self.agents.path_idx[a] as usize..])
            {
                continue;
            }
            if state == AgentState::Driving {
                let lane = self.agents.lane[a] as usize;
                self.traffic.queues[lane].retain(|&b| b != agent);
                self.agents.node[a] = self.roads.lane_to[lane];
                self.agents.cleared[a] = false;
            }
            self.totals.despawned_by_edit += 1;
            self.park(agent, RETRY_AFTER_S);
        }
        let state = &self.agents.state;
        self.entering
            .retain(|&agent| state[agent as usize] == AgentState::Entering);
    }

    pub(super) fn secs_to_ticks(&self, secs: f32) -> u64 {
        (secs / self.cfg.dt).round().max(1.0) as u64
    }

    fn park(&mut self, agent: AgentId, secs: f32) {
        if let Some(c) = self.agents.citizen[agent as usize] {
            self.retry_citizen_trip(c);
            return;
        }
        self.agents.state[agent as usize] = AgentState::Parked;
        let wake_at = self.totals.tick + self.secs_to_ticks(secs);
        self.wake.push(Reverse((wake_at, agent)));
    }

    pub(super) fn wake_parked(&mut self) {
        let nodes = self.roads.node_count() as u32;
        while let Some(&Reverse((at, agent))) = self.wake.peek() {
            if at > self.totals.tick {
                break;
            }
            self.wake.pop();
            let a = agent as usize;
            let mut dest = self.rng.below(nodes);
            if dest == self.agents.node[a] {
                dest = (dest + 1) % nodes;
            }
            self.agents.dest[a] = dest;
            self.agents.state[a] = AgentState::AwaitingPath;
            self.path_queue.push_back(agent);
        }
    }

    pub(super) fn plan_paths(&mut self) {
        for _ in 0..self.cfg.path_budget {
            let Some(agent) = self.path_queue.pop_front() else {
                break;
            };
            let a = agent as usize;
            // The path budget may defer a request across road edits. Resolve current access,
            // rather than routing from stale junctions retained in the vehicle slot.
            if let Some(c) = self.agents.citizen[a] {
                let i = c as usize;
                let from = self.building_node(self.citizens.location[i]);
                let to = self.citizens.destination[i].and_then(|b| self.building_node(b));
                let (Some(from), Some(to)) = (from, to) else {
                    self.retry_citizen_trip(c);
                    continue;
                };
                if from == to {
                    self.totals.trips_completed += 1;
                    self.citizen_arrived(c);
                    continue;
                }
                self.agents.node[a] = from;
                self.agents.dest[a] = to;
            }
            let mut path = std::mem::take(&mut self.agents.path[a]);
            let (from, to) = (self.agents.node[a], self.agents.dest[a]);
            // A vehicle slot belongs to one driver for life (ADR 0021), so its taste is stable.
            let driver = crate::rng::mix(self.cfg.seed ^ ROUTE_TASTE_SALT, u64::from(agent));
            let found = self.pathfinder.find(
                &self.roads,
                &self.lane_delay,
                Some(driver),
                from,
                to,
                &mut path,
            ) && !path.is_empty();
            self.agents.path[a] = path;
            self.totals.path_nodes_expanded += u64::from(self.pathfinder.last_expanded);
            if found {
                self.totals.paths_found += 1;
                self.agents.path_idx[a] = 0;
                self.agents.speed[a] = 0.0;
                self.agents.state[a] = AgentState::Entering;
                self.entering.push(agent);
            } else {
                self.totals.paths_failed += 1;
                self.park(agent, RETRY_AFTER_S);
            }
        }
    }

    pub(super) fn enter_lanes(&mut self) {
        let (traffic, agents) = (&mut self.traffic, &mut self.agents);
        self.entering.retain(|&agent| {
            let lane = agents.path[agent as usize][0];
            if traffic.has_room(lane, agents) {
                traffic.enter(lane, agent, agents);
                false
            } else {
                true
            }
        });
    }

    pub(super) fn resolve_exits(&mut self, exits: &[LaneId]) {
        let stuck_limit = self
            .secs_to_ticks(self.cfg.stuck_limit_s)
            .min(u16::MAX as u64) as u16;
        let detour_after = self.secs_to_ticks(DETOUR_AFTER_S) as u16;
        let detour_every = self.secs_to_ticks(DETOUR_EVERY_S).max(1) as u16;
        for &lane in exits {
            let l = lane as usize;
            let len = self.roads.lane_len[l];
            // The front car reached the lane end: it moves on, arrives, or waits for room.
            let front = self.traffic.queues[l][0];
            let f = front as usize;
            if self.agents.pos[f] >= len {
                let next_idx = self.agents.path_idx[f] as usize + 1;
                if next_idx >= self.agents.path[f].len() {
                    self.arrive(lane, front);
                } else {
                    let next = self.agents.path[f][next_idx];
                    if self.traffic.has_room(next, &self.agents) {
                        self.traffic.queues[l].pop_front();
                        self.agents.path_idx[f] = next_idx as u32;
                        self.traffic.enter(next, front, &mut self.agents);
                    } else {
                        self.held.push((front, lane, true));
                    }
                }
            }

            // The first car not yet cleared may be approaching the stop line, even while cars
            // ahead of it are still crossing, so a moving queue keeps moving.
            let Some(k) = self.traffic.first_waiting(lane, &self.agents) else {
                continue;
            };
            let agent = self.traffic.queues[l][k];
            let a = agent as usize;
            if self.agents.pos[a] >= len
                || !Traffic::wants_to_ask(
                    &self.roads,
                    &self.agents,
                    &self.junctions.stop,
                    lane,
                    agent,
                )
            {
                continue; // Handled above, or not there yet.
            }
            let at_line = self.agents.pos[a] >= self.junctions.stop[l];
            let next_idx = self.agents.path_idx[a] as usize + 1;
            let Some(&next) = self.agents.path[a].get(next_idx) else {
                if at_line && k == 0 {
                    self.arrive(lane, agent);
                }
                continue;
            };
            let verdict = self.junctions.request(
                &self.roads,
                &self.traffic,
                &self.agents,
                agent,
                lane,
                next,
                self.totals.tick,
                self.cfg.dt,
            );
            match verdict {
                Verdict::Go => self.agents.cleared[a] = true,
                Verdict::Wait if at_line => self.held.push((agent, lane, false)),
                Verdict::Wait | Verdict::Red => {}
            }
        }

        // Wait times only grow once every car has decided, so cars compare them as they were
        // at the start of the tick, whatever order their lanes are visited in. Otherwise two
        // cars that each think the other waited longer can wait for each other forever.
        let held = std::mem::take(&mut self.held);
        for &(agent, lane, at_end) in &held {
            let a = agent as usize;
            let next = self.agents.path[a][self.agents.path_idx[a] as usize + 1];
            self.agents.stuck_ticks[a] += 1;
            let waited = self.agents.stuck_ticks[a];
            if !at_end
                && waited >= detour_after
                && (waited - detour_after).is_multiple_of(detour_every)
                && !self
                    .junctions
                    .exit_clear(&self.roads, &self.traffic, &self.agents, next)
            {
                self.try_detour(agent, lane, next);
            }
            if waited >= stuck_limit {
                let queue = &mut self.traffic.queues[lane as usize];
                let k = queue
                    .iter()
                    .position(|&b| b == agent)
                    .expect("held car is queued");
                queue.remove(k);
                self.agents.node[a] = self.roads.lane_to[lane as usize];
                self.agents.cleared[a] = false;
                self.totals.despawned_stuck += 1;
                self.park(agent, RETRY_AFTER_S);
            }
        }
        self.held = held;
        self.held.clear();
    }

    /// Ends the trip of `agent`, the front car on `lane`.
    fn arrive(&mut self, lane: LaneId, agent: AgentId) {
        debug_assert_eq!(self.traffic.queues[lane as usize].front(), Some(&agent));
        self.traffic.queues[lane as usize].pop_front();
        let a = agent as usize;
        self.agents.node[a] = self.agents.dest[a];
        self.totals.trips_completed += 1;
        if let Some(c) = self.agents.citizen[a] {
            self.citizen_arrived(c);
            return;
        }
        let dwell = self.rng.range(self.cfg.dwell_min_s, self.cfg.dwell_max_s);
        self.park(agent, dwell);
    }

    /// Reroutes a car at `lane`'s stop line around its blocked `next` lane, keeping its place in
    /// the queue. Rings of full lanes need every car to want the lane ahead; one turning off
    /// breaks the ring. The search is local (around the block, back onto the old route), so it
    /// costs a few dozen nodes instead of a city-wide search. Keeps the old route if nothing
    /// nearby works.
    fn try_detour(&mut self, agent: AgentId, lane: LaneId, next: LaneId) {
        let a = agent as usize;
        let idx = self.agents.path_idx[a] as usize;
        let route = &self.agents.path[a][idx + 1..];
        debug_assert_eq!(route.first(), Some(&next));
        debug_assert_eq!(
            self.roads.lane_to[lane as usize],
            self.roads.lane_from[next as usize]
        );
        let (roads, traffic, agents, junctions) =
            (&self.roads, &self.traffic, &self.agents, &self.junctions);
        let Some(j) = self.pathfinder.rejoin(
            roads,
            &self.lane_delay,
            route,
            |first| junctions.exit_clear(roads, traffic, agents, first),
            DETOUR_MAX_NODES,
            &mut self.detour,
        ) else {
            return;
        };
        self.detour.extend_from_slice(&route[j + 1..]);
        let path = &mut self.agents.path[a];
        path.truncate(idx + 1);
        path.extend_from_slice(&self.detour);
        self.totals.detours += 1;
    }
}
