// The one-line hint in the build menu's options row, shown while a tool is in use.
import { type Price, type Prices, Zone } from '../protocol';
import type { Tool } from './catalog';
import { dollars } from './figures';

/** The sim's quote for the previewed road (ADR 0036), and how many existing roads it would
 * change to the drawn class (ADR 0048). */
export type Quote = { cost: number | null; affordable: boolean; changes?: number } | null;

/** Names the roads a draft would change to its class; drawing a main road only upgrades. */
function changeText(quote: Quote, main: boolean) {
  const n = quote?.changes ?? 0;
  if (!n) return '';
  const roads = n === 1 ? '1 road' : `${n} roads`;
  return `${main ? 'Upgrades' : 'Changes'} ${roads} to ${main ? 'main road' : 'street'}. `;
}

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
  gridStart: 'Click where the grid starts.',
  edge: 'Click the end of the first edge to set the angle.',
  area: 'Move to stretch the grid in whole blocks, then click to build.',
};

/** Help for the road and bulldoze tools; empty when neither is active. */
export function roadHelpText(
  tool: 'straight' | 'curve' | 'continuous' | 'grid' | 'upgrade' | 'bulldoze' | null,
  stage: keyof typeof STAGES,
  building: boolean,
  main: boolean,
  refused: boolean,
  quote: Quote,
) {
  if (tool === 'bulldoze')
    return 'Click a highlighted road, building or power piece to remove it. Esc or right-click cancels.';
  if (!tool) return '';
  if (tool === 'upgrade')
    return `${costText(quote)}Click a road to make it a ${main ? 'main road' : 'street'}. Esc or right-click cancels.`;
  if (building) return tool === 'grid' ? 'Building grid…' : 'Building road…';
  const problem = refused
    ? tool === 'continuous'
      ? 'Cannot build here. Move the end ahead, away from overlaps or short joins. '
      : tool === 'grid'
        ? 'Nothing to build here. Move the grid off the roads in its way. '
        : 'Cannot build here. Move the end away from overlaps or short joins. '
    : `${changeText(quote, main)}${costText(quote)}`;
  return `${main ? 'Main road. ' : ''}${problem}${STAGES[stage]} Hold Shift to place freely. Esc or right-click cancels.`;
}

/** Help for the zone brushes; empty when none is active. */
export function zoneHelpText(zone: Zone | null) {
  if (zone === null) return '';
  if (zone === Zone.None)
    return 'Click a block or drag a rectangle to remove zoning. Esc or right-click stops.';
  return 'Click a block to fill it, or drag a rectangle to paint. Press the zone key again for the other density. Esc or right-click stops.';
}

/** What an item costs, for its tooltip: build cost and upkeep, or Free; null for roads and
 * zones, whose cost depends on what is drawn. */
export function priceText(price: Price | 'free' | null) {
  if (price === null) return null;
  if (price === 'free') return 'Free.';
  return `Costs ${dollars(price.cost)} to build and ${dollars(price.upkeep)} an hour to keep.`;
}

/** The price of what an item builds; see `priceText`. */
export function itemPrice(prices: Prices, tool: Tool): Price | 'free' | null {
  switch (tool.kind) {
    case 'plant':
      return prices.plants[tool.plant];
    case 'water':
      return prices.water[tool.water];
    case 'station':
      return prices.station;
    case 'line':
      return 'free';
    default:
      return null;
  }
}

/** The start of a placement tool's hint: its name and price, or that the city cannot pay. */
export function placeText(name: string, price: Price | undefined, affordable: boolean) {
  if (!price) return `${name}:`;
  if (!affordable) return `Not enough money: a ${name.toLowerCase()} costs ${dollars(price.cost)}.`;
  return `${name}: ${dollars(price.cost)}, then ${dollars(price.upkeep)} an hour.`;
}

/** Why a locked tool cannot be used yet (ADR 0056): it waits for the town. */
export function lockedText(needed: number): string {
  const more =
    needed === 1 ? '1 more resident' : `${needed.toLocaleString('en-US')} more residents`;
  return `Unlocks in the town era, ${more} needed.`;
}
