import type { Stats, Zone } from './protocol';

const fmt = (n: number) => n.toLocaleString('en-US');
/** Whole dollars with the sign in front: `$1,234`, `-$56`. */
export const dollars = (n: number) => `${n < 0 ? '-' : ''}$${fmt(Math.abs(n))}`;

/** The sim's quote for the previewed road (ADR 0036). */
export type Quote = { cost: number | null; affordable: boolean } | null;

function costText(quote: Quote) {
  if (!quote || quote.cost === null) return '';
  return quote.affordable
    ? `Costs ${dollars(quote.cost)}. `
    : `Not enough money: costs ${dollars(quote.cost)}. `;
}

const STAGES = {
  start: 'Click the road start.',
  control: 'Click the bend control point, then the road end.',
  end: 'Click the road end.',
};

/** Help for the road and bulldoze tools; empty when neither is active. */
export function roadHelpText(
  tool: 'straight' | 'curve' | 'bulldoze' | null,
  stage: keyof typeof STAGES,
  building: boolean,
  main: boolean,
  refused: boolean,
  quote: Quote,
) {
  if (tool === 'bulldoze')
    return 'Click a highlighted road to remove it. Esc or right-click cancels.';
  if (!tool) return '';
  if (building) return 'Building road…';
  const problem = refused
    ? 'Cannot build here. Move the end away from overlaps or short joins. '
    : costText(quote);
  return `${main ? 'Main road. ' : ''}${problem}${STAGES[stage]} Hold Shift to place freely. Esc or right-click cancels.`;
}
const ZONE_NAMES = [
  'unzone',
  'residential low',
  'residential high',
  'commercial low',
  'commercial high',
  'industrial low',
  'industrial high',
];
/** Display-only time, traffic and active-tool information. */
export function hudText(s: Stats, fps: number, tickMs: number, speed: number, zone: Zone | null) {
  const walkShare = s.tripsCompleted
    ? ((100 * s.walksCompleted) / s.tripsCompleted).toFixed(0)
    : '0';
  const hh = String(Math.floor(s.secondOfDay / 3600)).padStart(2, '0');
  const mm = String(Math.floor((s.secondOfDay % 3600) / 60)).padStart(2, '0');
  return (
    `Day ${s.day}   ${hh}:${mm}   ${speed === 0 ? 'paused' : `${speed}×`}\n` +
    `fps        ${fps.toFixed(0)}   tick ${tickMs.toFixed(2)} ms\n` +
    `money      ${dollars(s.balance)}   ${s.taxes - s.upkeep < 0 ? '-' : '+'}${dollars(Math.abs(s.taxes - s.upkeep))}/h (taxes ${dollars(s.taxes)}, upkeep ${dollars(s.upkeep)})${s.balance < 0 ? '   in debt: no new roads' : ''}\n` +
    `population ${fmt(s.population)}   unemployed ${fmt(s.unemployed)} (${s.unemploymentPercent.toFixed(1)}%)\n` +
    `driving    ${fmt(s.driving)}   walking ${fmt(s.walking)}   parked ${fmt(s.parked)}\n` +
    `trips      ${fmt(s.tripsCompleted)}   on foot ${walkShare}%\n` +
    `stuck      ${fmt(s.despawnedStuck)}   no path ${fmt(s.pathsFailed)}\n` +
    `road edits ${fmt(s.despawnedByEdit)} cars removed\n` +
    `camera     WASD or drag pans, Q/E turns, right-drag orbits, wheel zooms\n` +
    'inspect    click building/car/walker; F follows, Esc closes   N sound   T traffic\n' +
    (zone === null
      ? ''
      : `\nZONE: ${ZONE_NAMES[zone]}.  Click a block to fill it, drag a rectangle to paint, Esc to stop` +
        '\n(Z/X/C pick a zone, press again for the other density; U unzones)')
  );
}
