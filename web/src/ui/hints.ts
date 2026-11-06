// The one-line hint in the build menu's options row, shown while a tool is in use.
import { Zone } from '../protocol';
import { dollars } from './figures';

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
  tool: 'straight' | 'curve' | 'continuous' | 'bulldoze' | null,
  stage: keyof typeof STAGES,
  building: boolean,
  main: boolean,
  refused: boolean,
  quote: Quote,
) {
  if (tool === 'bulldoze')
    return 'Click a highlighted road, building or power piece to remove it. Esc or right-click cancels.';
  if (!tool) return '';
  if (building) return 'Building road…';
  const problem = refused
    ? tool === 'continuous'
      ? 'Cannot build here. Move the end ahead, away from overlaps or short joins. '
      : 'Cannot build here. Move the end away from overlaps or short joins. '
    : costText(quote);
  return `${main ? 'Main road. ' : ''}${problem}${STAGES[stage]} Hold Shift to place freely. Esc or right-click cancels.`;
}

/** Help for the zone brushes; empty when none is active. */
export function zoneHelpText(zone: Zone | null) {
  if (zone === null) return '';
  if (zone === Zone.None)
    return 'Click a block or drag a rectangle to remove zoning. Esc or right-click stops.';
  return 'Click a block to fill it, or drag a rectangle to paint. Press the zone key again for the other density. Esc or right-click stops.';
}
