// The inspector's answers (ADR 0022, 0029): the worker asks the sim about one citizen or
// building and converts names, nothing more.
import type { Sim } from './pkg/sim_wasm';
import type { Building, Citizen, Place } from './protocol';

function place(building: number | undefined, zone: number | undefined): Place | null {
  return building === undefined ? null : { building, zone: zone as Place['zone'] };
}

/** The sim's details of citizen `id`; null if there is no such citizen. */
export function readCitizen(s: Sim, id: number): Citizen | null {
  const v = s.citizen_info(id);
  if (!v) return null;
  const citizen: Citizen = {
    id,
    name: v.name,
    resident: v.resident,
    home: place(v.home, v.home_zone),
    job: place(v.job, v.job_zone),
    activity: v.activity as Citizen['activity'],
    location: place(v.location, v.location_zone),
    destination: place(v.destination, v.destination_zone),
    agent: v.agent ?? null,
    driving: v.driving,
    onFoot: v.on_foot,
    age: v.age,
    child: v.child,
    grewUp: v.grew_up,
    wantsHome: v.wants_home,
    mood: v.mood,
    complaints: Array.from(v.complaints) as Citizen['complaints'],
    commute: v.commute,
    education: v.education as Citizen['education'],
    school: place(v.school, v.school_zone),
    riding: v.riding,
  };
  v.free();
  return citizen;
}

/** The sim's details of building `id`; null once it is gone. */
export function readBuilding(s: Sim, id: number): Building | null {
  const v = s.building_info(id);
  if (!v) return null;
  const building: Building = {
    building: v.building,
    zone: v.zone as Building['zone'],
    capacity: v.capacity,
    occupied: v.occupied,
    closing: v.closing,
    powered: v.powered,
    watered: v.watered,
    seats: v.seats ?? null,
    pupils: v.pupils ?? null,
  };
  v.free();
  return building;
}
