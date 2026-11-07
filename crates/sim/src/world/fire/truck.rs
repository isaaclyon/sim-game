//! Fire trucks (ADR 0053): agents that drive the road network from their station to a fire's
//! frontage junction and back. Trip planning, lanes and queues are the cars' own; only the
//! end of a trip, a lost route and the junction rule differ.

use super::Task;
use crate::agents::{AgentId, AgentState};
use crate::building::BuildingId;
use crate::road::NodeId;
use crate::world::World;

impl World {
    fn home_of(&self, t: usize) -> Option<NodeId> {
        self.fire
            .cover
            .node
            .get(self.fire.truck_station[t] as usize)
            .copied()
            .flatten()
    }

    /// Sends free truck `t` to building `b`'s junction `to`.
    pub(super) fn send_truck(&mut self, t: usize, to: NodeId) {
        let agent = self.fire.truck_agent[t];
        let a = agent as usize;
        self.fire.truck_task[t] = Task::Outbound;
        self.agents.node[a] = self.home_of(t).unwrap_or(to);
        self.agents.dest[a] = to;
        self.agents.state[a] = AgentState::AwaitingPath;
        self.path_queue.push_back(agent);
    }

    /// Parks truck `t` at its station, free, wherever it was.
    fn truck_home(&mut self, t: usize) {
        let a = self.fire.truck_agent[t] as usize;
        if let Some(home) = self.home_of(t) {
            self.agents.node[a] = home;
        }
        self.agents.state[a] = AgentState::Dormant;
        self.agents.cleared[a] = false;
        self.fire.truck_task[t] = Task::Home;
    }

    /// Takes truck `t` out of every traffic list; it ends dormant.
    pub(super) fn take_off_road(&mut self, t: usize) {
        let agent = self.fire.truck_agent[t];
        let a = agent as usize;
        if self.agents.state[a] == AgentState::Driving {
            let lane = self.agents.lane[a] as usize;
            self.traffic.queues[lane].retain(|&v| v != agent);
        }
        self.entering.retain(|&v| v != agent);
        self.path_queue.retain(|&v| v != agent);
        self.truck_home(t);
    }

    /// Drops truck `t` from the fire it was sent to, if any.
    pub(super) fn unassign(&mut self, t: usize) {
        for f in &mut self.fire.fire_truck {
            if *f == Some(t as u32) {
                *f = None;
            }
        }
    }

    /// Turns truck `t` back to its station: at once if it has not left, else from the end of
    /// the lane it is on.
    pub(super) fn turn_back(&mut self, t: usize) {
        let agent = self.fire.truck_agent[t];
        let a = agent as usize;
        if self.agents.state[a] != AgentState::Driving {
            self.take_off_road(t);
            return;
        }
        self.fire.truck_task[t] = Task::Returning;
        let Some(home) = self.home_of(t) else {
            return;
        };
        let idx = self.agents.path_idx[a] as usize;
        let lane = self.agents.path[a][idx];
        self.agents.path[a].truncate(idx + 1);
        self.agents.dest[a] = home;
        let from = self.roads.lane_to[lane as usize];
        let mut back = Vec::new();
        if from != home
            && self
                .pathfinder
                .find(&self.roads, &self.lane_delay, None, from, home, &mut back)
        {
            self.agents.path[a].extend(back);
        }
    }

    /// Called when a truck reaches the end of its route.
    pub(in crate::world) fn truck_arrived(&mut self, agent: AgentId) {
        let Some(t) = self.fire.truck_of(agent) else {
            return;
        };
        let a = agent as usize;
        match self.fire.truck_task[t] {
            Task::Outbound => {
                // Returning first, so putting the fire out does not turn the truck back.
                self.fire.truck_task[t] = Task::Returning;
                let fire = self
                    .fire
                    .fire_truck
                    .iter()
                    .position(|&f| f == Some(t as u32));
                if let Some(f) = fire {
                    let b = self.fire.fire_building[f];
                    self.put_out(b);
                }
                match self.home_of(t) {
                    Some(home) if home != self.agents.node[a] => {
                        self.agents.dest[a] = home;
                        self.agents.state[a] = AgentState::AwaitingPath;
                        self.path_queue.push_back(agent);
                    }
                    _ => self.truck_home(t),
                }
            }
            _ => self.truck_home(t),
        }
    }

    /// Called instead of parking a truck whose route was lost: no path, stuck, or a road
    /// edit. Its fire waits for another truck and it goes home.
    pub(in crate::world) fn truck_lost(&mut self, agent: AgentId) {
        if let Some(t) = self.fire.truck_of(agent) {
            self.unassign(t);
            self.truck_home(t);
        }
    }

    /// Saves building `b`: the truck sent to it turns for home.
    fn put_out(&mut self, b: BuildingId) {
        if let Some(f) = self.fire.burning(b) {
            self.drop_fire(f);
            self.fire.saved += 1;
        }
    }

    /// Ends fire `f` without a verdict, turning back its truck if it is still on the way.
    pub(super) fn drop_fire(&mut self, f: usize) {
        let truck = self.fire.fire_truck[f];
        self.fire.fire_building.remove(f);
        self.fire.fire_until.remove(f);
        self.fire.fire_truck.remove(f);
        self.fire.bump();
        if let Some(t) = truck.map(|t| t as usize)
            && self.fire.truck_task[t] == Task::Outbound
        {
            self.turn_back(t);
        }
    }

    /// Ends the fire on building `b`, if it burns, saving nothing and losing nothing.
    pub(super) fn end_fire(&mut self, b: BuildingId) {
        if let Some(f) = self.fire.burning(b) {
            self.drop_fire(f);
        }
    }
}
