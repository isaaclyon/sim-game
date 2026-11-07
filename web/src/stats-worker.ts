// The sim's figures for the frame's `stats`: the worker converts names, nothing more.
import { readFireStats } from './fire-worker';
import type { Sim } from './pkg/sim_wasm';
import type { Stats } from './protocol';

/** The sim's figures now; see `Stats`. */
export function readStats(s: Sim): Stats {
  const st = s.stats();
  const out: Stats = {
    tick: st.tick,
    day: st.day,
    secondOfDay: st.second_of_day,
    agents: st.agents,
    population: st.population,
    children: st.children,
    unemployed: st.unemployed,
    unemploymentPercent: st.unemployment_percent,
    demand: {
      residential: st.demand_residential,
      commercial: st.demand_commercial,
      industrial: st.demand_industrial,
    },
    power: {
      supply: st.power_supply,
      starter: st.power_starter,
      used: st.power_used,
      demand: st.power_demand,
      unpowered: st.power_unpowered,
      waiting: st.power_waiting,
    },
    water: {
      supply: st.water_supply,
      used: st.water_used,
      demand: st.water_demand,
      stored: st.water_stored,
      capacity: st.water_capacity,
      dry: st.water_dry,
      waiting: st.water_waiting,
    },
    parked: st.parked,
    awaitingPath: st.awaiting_path,
    entering: st.entering,
    driving: st.driving,
    walking: st.walking,
    tripsCompleted: st.trips_completed,
    walksCompleted: st.walks_completed,
    pathsFailed: st.paths_failed,
    despawnedStuck: st.despawned_stuck,
    despawnedByEdit: st.despawned_by_edit,
    balance: st.balance,
    taxes: st.taxes,
    grant: st.grant,
    upkeep: st.upkeep,
    utilities: st.utilities,
    fire: readFireStats(s),
  };
  st.free();
  return out;
}
