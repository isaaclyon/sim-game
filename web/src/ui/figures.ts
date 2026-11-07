// Words and numbers for the top bar, its detail panels and the debug panel. Display only: every
// figure comes from the sim's stats or the worker's save status.
import type {
  CityStatus,
  Demand,
  FireStats,
  PowerStats,
  Stats,
  WaterStats,
} from '../protocol';

export const fmt = (n: number) => n.toLocaleString('en-US');
/** Whole dollars with the sign in front: `$1,234`, `-$56`. */
export const dollars = (n: number) => `${n < 0 ? '-' : ''}$${fmt(Math.abs(n))}`;
/** A rate per clock hour with its sign: `+$180/h`, `-$40/h`. */
export const perHour = (n: number) => `${n < 0 ? '-' : '+'}${dollars(Math.abs(n))}/h`;
const percent = (part: number, whole: number) => (whole ? (100 * part) / whole : 0);

export function clockText(s: Pick<Stats, 'day' | 'secondOfDay'>) {
  const hh = String(Math.floor(s.secondOfDay / 3600)).padStart(2, '0');
  const mm = String(Math.floor((s.secondOfDay % 3600) / 60)).padStart(2, '0');
  return `Day ${s.day} · ${hh}:${mm}`;
}

/** The population panel's rows, in the order the page lists them. */
export function populationRows(s: Stats): Record<string, string> {
  return {
    'pop-residents': fmt(s.population),
    'pop-unemployed': `${fmt(s.unemployed)} (${s.unemploymentPercent.toFixed(1)}%)`,
    'pop-driving': fmt(s.driving),
    'pop-walking': fmt(s.walking),
    'pop-parked': fmt(s.parked),
    'pop-trips': `${fmt(s.tripsCompleted)} · ${percent(s.walksCompleted, s.tripsCompleted).toFixed(0)}% on foot`,
  };
}

/** Power needed against power available (ADR 0041), whether the town is short, and whether
 * to warn: also while lots wait for power to grow. */
export function powerFigures(p: PowerStats) {
  const available = p.supply + p.starter;
  const short = p.demand > available;
  const warning = short || p.waiting > 0;
  const share = available > 0 ? Math.round(percent(p.demand, available)) : p.demand > 0 ? null : 0;
  return { available, short, warning, text: share === null ? 'No power' : `${fmt(share)}%` };
}

/** One line on what to do about power. */
export function powerAdvice(p: PowerStats) {
  if (powerFigures(p).short) return 'Build a plant, or join a grid to one with a power line.';
  if (p.unpowered)
    return 'Some buildings are on a grid without enough power. Join it to a powered road with a power line.';
  if (p.waiting) return 'New buildings need more power to grow. Build a plant.';
  return 'Every building has power.';
}

/** Water use against pump output (ADR 0052); short when a dense building is dry. */
export function waterFigures(w: WaterStats) {
  const short = w.dry > 0;
  const share = w.supply > 0 ? Math.round(percent(w.demand, w.supply)) : w.demand > 0 ? null : 0;
  return { short, text: share === null ? 'No water' : `${fmt(share)}%` };
}

/** One line on what to do about water. */
export function waterAdvice(w: WaterStats) {
  if (w.dry) return 'Some dense buildings are dry. Build a pump on a road their network reaches.';
  if (w.demand > w.supply) return 'Towers are draining. Build another pump before they run dry.';
  if (w.waiting) return 'New dense buildings need more water to grow. Build a pump.';
  return 'Every dense building has water. Low density needs none.';
}

/** The fire figure warns from this share of the fire threshold while the city has no station. */
const FIRE_NEAR = 0.8;

/**
 * The top bar's fire figure (ADR 0053). `alarm` (red) while a fire burns with no truck on the
 * way; `near` (amber) as a city without a station nears the population where fires start.
 */
export function fireFigures(f: FireStats, population: number) {
  const alarm = f.enabled && f.unattended > 0;
  const below = population < f.threshold;
  const near = f.enabled && !alarm && f.stations === 0 && population >= f.threshold * FIRE_NEAR;
  const text = !f.enabled
    ? 'Off'
    : f.burning > 0
      ? `${fmt(f.burning)} burning`
      : f.stations > 0
        ? `${fmt(f.stations)} ${f.stations === 1 ? 'station' : 'stations'}`
        : below
          ? near
            ? `Fires at ${fmt(f.threshold)}`
            : 'No fires yet'
          : 'No station';
  const advice = !f.enabled
    ? 'Fires are off for this city.'
    : alarm
      ? 'A fire burns with no truck on the way. Build a fire station near it, or clear the traffic in the way.'
      : below
        ? `Buildings start to catch fire once ${fmt(f.threshold)} people live here. Build a fire station beside a road before then.`
        : f.stations === 0
          ? 'Buildings can catch fire. Build a fire station beside a road.'
          : 'Stations send trucks along the roads. Buildings they cannot reach in time burn down.';
  return { alarm, near, text, advice };
}

/** Signed share of a bar's half-width for a demand in people; never overflows the track. */
export function demandFill(people: number): number {
  return people / (Math.abs(people) + 60);
}

export function demandLabel(demand: Demand) {
  const parts = (['residential', 'commercial', 'industrial'] as const).map(
    (use) => `${use} ${demand[use] > 0 ? '+' : ''}${demand[use]}`,
  );
  return `demand: ${parts.join(', ')}`;
}

function ago(ms: number) {
  if (ms < 60_000) return 'just now';
  if (ms < 3_600_000) return `${Math.floor(ms / 60_000)}m ago`;
  return `${Math.floor(ms / 3_600_000)}h ago`;
}

/** The short save status in the top bar; red only for an error. While the clock runs the city
 * always has changes since its last save, so the time of that save is what it shows. */
export function saveBadge(s: CityStatus | null, now: number): { text: string; error: boolean } {
  if (!s || (s.busy && !s.ready)) return { text: 'Opening city…', error: false };
  if (s.busy) return { text: 'Saving…', error: false };
  if (s.error) return { text: s.error, error: true };
  if (s.savedAt === null) return { text: 'Not saved yet', error: false };
  return { text: `Saved ${ago(now - s.savedAt)}`, error: false };
}

/** Developer numbers for the debug panel (the backtick key). */
export function debugText(s: Stats, fps: number, tickMs: number, picturesMs = 0) {
  return [
    `fps            ${fps.toFixed(0)}`,
    `tick           ${tickMs.toFixed(2)} ms`,
    `agents         ${fmt(s.agents)}   awaiting path ${fmt(s.awaitingPath)}   entering ${fmt(s.entering)}`,
    `stuck cars     ${fmt(s.despawnedStuck)}`,
    `failed routes  ${fmt(s.pathsFailed)}`,
    `road edits     ${fmt(s.despawnedByEdit)} cars removed`,
    `menu pictures  ${picturesMs.toFixed(0)} ms at startup`,
  ].join('\n');
}
