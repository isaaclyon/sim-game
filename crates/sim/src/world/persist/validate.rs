//! Cross-checks between the parts of a decoded world (ADR 0033). Each part checks itself;
//! this checks that the IDs one part holds name slots that exist in another.

use std::cmp::Reverse;

use crate::agents::AgentState;
use crate::check::{Check, below, ensure, finite, lens};
use crate::world::World;
use crate::zone::Zone;

impl World {
    pub(super) fn validate(&self) -> Check {
        let c = &self.cfg;
        ensure(c.dt.is_finite() && c.dt > 0.0, "tick length")?;
        ensure(
            c.dwell_min_s.is_finite()
                && c.dwell_max_s.is_finite()
                && (0.0..=c.dwell_max_s).contains(&c.dwell_min_s),
            "dwell range",
        )?;
        ensure(
            c.stuck_limit_s.is_finite() && c.stuck_limit_s >= 0.0 && c.grid_spacing.is_finite(),
            "config values",
        )?;
        ensure((0..60).contains(&self.treasury.carry), "money carry")?;
        let z = &self.zones;
        finite([&z.x0, &z.y0], "zone origin")?;
        self.roads.check()?;
        self.zones.check()?;
        self.lots
            .check(self.zones.cells.len(), self.roads.lane_count() / 2)?;
        self.buildings.check(self.lots.len())?;
        self.power.check()?;
        self.water.check(self.buildings.active.len())?;
        self.fire.check(
            self.agents.len(),
            self.buildings.active.len(),
            self.lots.len(),
        )?;
        self.pollution
            .check(&self.zones, self.roads.lane_count() / 2)?;
        self.junctions.check(&self.roads)?;
        self.traffic
            .check(self.roads.lane_count(), self.agents.len())?;
        self.check_agents()?;
        self.check_people()?;
        self.check_schedules()
    }

    fn check_agents(&self) -> Check {
        let a = &self.agents;
        let (n, lanes) = (a.len(), self.roads.lane_count());
        let fields = [
            a.citizen.len(),
            a.node.len(),
            a.dest.len(),
            a.lane.len(),
            a.pos.len(),
            a.speed.len(),
            a.path.len(),
            a.path_idx.len(),
            a.stuck_ticks.len(),
            a.cleared.len(),
            a.walk_end.len(),
        ];
        lens(&fields, n, "agent fields")?;
        let nodes = self.roads.node_count();
        below(a.node.iter().chain(&a.dest), nodes, "agent nodes")?;
        // A fresh slot points at lane 0 even before any road exists.
        ensure(
            a.lane.iter().all(|&l| l == 0 || (l as usize) < lanes),
            "agent lanes",
        )?;
        below(a.path.iter().flatten(), lanes, "agent routes")?;
        ensure(
            a.path_idx
                .iter()
                .zip(&a.path)
                .all(|(&i, p)| i as usize <= p.len()),
            "route progress",
        )?;
        below(
            a.citizen.iter().flatten(),
            self.citizens.active.len(),
            "agent citizens",
        )?;
        finite(
            a.pos.iter().chain(&a.speed).chain(&a.walk_end),
            "agent motion",
        )?;
        for i in 0..n {
            let travelling = matches!(
                a.state[i],
                AgentState::Entering | AgentState::Driving | AgentState::Walking
            );
            let on_lane = matches!(a.state[i], AgentState::Driving | AgentState::Walking);
            ensure(
                !travelling || (a.path_idx[i] as usize) < a.path[i].len(),
                "travelling agent routes",
            )?;
            ensure(
                !on_lane || (a.lane[i] as usize) < lanes,
                "travelling agent lanes",
            )?;
            ensure(
                a.state[i] != AgentState::Walking || a.citizen[i].is_some(),
                "walker citizens",
            )?;
        }
        // Drivers and lane queues match one to one.
        let mut queued = vec![false; n];
        for (lane, queue) in self.traffic.queues.iter().enumerate() {
            for &agent in queue {
                let i = agent as usize;
                ensure(
                    !queued[i] && a.state[i] == AgentState::Driving && a.lane[i] as usize == lane,
                    "lane queue drivers",
                )?;
                queued[i] = true;
            }
        }
        ensure(
            (0..n).all(|i| queued[i] == (a.state[i] == AgentState::Driving)),
            "unqueued drivers",
        )?;
        let queued = self
            .path_queue
            .iter()
            .chain(&self.entering)
            .chain(&self.walking);
        below(queued, n, "queued agents")?;
        below(
            self.wake.iter().map(|Reverse((_, a))| a),
            n,
            "parked agents",
        )
    }

    fn check_people(&self) -> Check {
        let (c, hh, homes) = (&self.citizens, &self.households, &self.homes);
        let (n, buildings) = (c.active.len(), self.buildings.active.len());
        let fields = [
            c.household.len(),
            c.job.len(),
            c.location.len(),
            c.destination.len(),
            c.activity.len(),
            c.agent.len(),
            c.due.len(),
            c.job_due.len(),
            c.departure.len(),
            c.work_duration.len(),
            c.owns_car.len(),
            c.car_out.len(),
            c.walk_limit.len(),
            c.walk_speed.len(),
        ];
        lens(&fields, n, "citizen fields")?;
        finite(c.walk_limit.iter().chain(&c.walk_speed), "citizen walking")?;
        below(&c.household, hh.active.len(), "citizen households")?;
        let places = c.job.iter().chain(&c.destination).flatten();
        below(places.chain(&c.location), buildings, "citizen places")?;
        below(
            c.agent.iter().flatten(),
            self.agents.len(),
            "citizen agents",
        )?;
        lens(
            &[hh.home.len(), hh.members.len()],
            hh.active.len(),
            "household fields",
        )?;
        below(&hh.home, homes.building.len(), "household homes")?;
        below(hh.members.iter().flatten(), n, "household members")?;
        let home_fields = [homes.capacity.len(), homes.household.len()];
        lens(&home_fields, homes.building.len(), "home fields")?;
        below(&homes.building, buildings, "home buildings")?;
        // Building inspection relies on homes being registered in building order.
        ensure(homes.building.is_sorted(), "home order")?;
        below(
            homes.household.iter().flatten(),
            hh.active.len(),
            "home households",
        )
    }

    fn check_schedules(&self) -> Check {
        let r = &self.residents;
        let (buildings, homes) = (self.buildings.active.len(), self.homes.building.len());
        ensure(r.registered <= buildings, "registered buildings")?;
        lens(&[r.jobs.len(), r.workers.len()], r.registered, "job counts")?;
        self.check_counts()?;
        if let Some(version) = r.connectivity_version {
            let nodes = self.roads.node_count();
            let current = version == self.roads.version;
            let len = r.component.len();
            ensure(
                len == nodes || (!current && len <= nodes),
                "road components",
            )?;
            below(&r.component, len, "road component ids")?;
        }
        below(
            r.arrivals.iter().map(|Reverse((_, h))| h),
            homes,
            "home arrivals",
        )?;
        below(&r.vacant, homes, "vacant homes")?;
        let citizens = self.citizens.active.len();
        let people = r.activities.iter().chain(&r.searches);
        below(
            people.map(|Reverse((_, c))| c),
            citizens,
            "citizen schedules",
        )?;
        let closing = self.closures.due.len();
        ensure(closing <= buildings, "closure steps")?;
        below(
            self.closures.heap.iter().map(|Reverse((_, b))| b),
            closing,
            "closing buildings",
        )
    }

    /// Counters that later ticks decrement or subtract must agree with what they count. Job
    /// supply totals are the jobs of exactly the counted buildings.
    fn check_counts(&self) -> Check {
        let (r, c) = (&self.residents, &self.citizens);
        let active = || (0..c.active.len()).filter(|&i| c.active[i]);
        ensure(r.population as usize == active().count(), "population")?;
        let employed = active().filter(|&i| c.job[i].is_some()).count();
        ensure(r.employed as usize == employed, "employment")?;
        let mut workers = vec![0u32; r.registered];
        for &job in c.job.iter().flatten() {
            ensure((job as usize) < r.registered, "unregistered jobs")?;
            workers[job as usize] += 1;
        }
        ensure(workers == r.workers, "worker counts")?;
        ensure(
            r.workers.iter().zip(&r.jobs).all(|(w, j)| w <= j),
            "workers within jobs",
        )?;
        let supply = &self.supply;
        ensure(supply.counted.len() <= r.registered, "counted supply")?;
        let (mut commercial, mut industrial) = (0u64, 0u64);
        for b in (0..supply.counted.len()).filter(|&b| supply.counted[b]) {
            match self.buildings.zone[b] {
                Zone::CommercialLow | Zone::CommercialHigh => commercial += u64::from(r.jobs[b]),
                Zone::IndustrialLow | Zone::IndustrialHigh => industrial += u64::from(r.jobs[b]),
                _ => {}
            }
        }
        ensure(
            (commercial, industrial)
                == (
                    u64::from(supply.commercial_jobs),
                    u64::from(supply.industrial_jobs),
                ),
            "job supply",
        )
    }
}
