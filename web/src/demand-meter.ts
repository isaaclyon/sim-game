import type { Demand } from './protocol';

const USES = ['residential', 'commercial', 'industrial'] as const;

/** Signed share of a bar's half-height for a demand in people; never overflows the track. */
export function demandFill(people: number): number {
  return people / (Math.abs(people) + 60);
}

/** Draws R/C/I demand (ADR 0024): a bar rises above the midline for demand, drops below it
 *  for a surplus. `fills` are the residential, commercial and industrial bars in that order. */
export function drawDemand(root: HTMLElement, fills: HTMLElement[], demand: Demand) {
  const parts = USES.map((use, i) => {
    const f = demandFill(demand[use]);
    const style = fills[i].style;
    style.height = `${(Math.abs(f) * 50).toFixed(1)}%`;
    style.bottom = f >= 0 ? '50%' : `${(50 - Math.abs(f) * 50).toFixed(1)}%`;
    fills[i].classList.toggle('surplus', f < 0);
    return `${use} ${demand[use] > 0 ? '+' : ''}${demand[use]}`;
  });
  const label = `demand: ${parts.join(', ')}`;
  root.setAttribute('aria-label', label);
  root.title = label;
}
