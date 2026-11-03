import type { Stats, Zone } from './protocol';

const fmt = (n: number) => n.toLocaleString('en-US');
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
