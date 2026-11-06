// The one list behind the build menu (M4.6): categories in build order, their items and the
// options of their tools. The menu is built from it, so a new tool is one entry here and one
// picture model in `pictures.ts`. Keys are shown in tooltips; main.ts handles them.
import { ZONE_OVERLAY } from '../palette';
import { PlantKind, RoadClass, Snap, Zone } from '../protocol';

export type RoadMode = 'straight' | 'curve' | 'continuous';

/** What an item switches on. */
export type Tool =
  | { kind: 'road'; roadClass: RoadClass }
  | { kind: 'zone'; zone: Zone }
  | { kind: 'plant'; plant: PlantKind }
  | { kind: 'line' }
  | { kind: 'bulldoze' };

/** What an option changes on the tool in use. */
export type OptionAction = { kind: 'roadMode'; mode: RoadMode } | { kind: 'snap'; flag: number };

/** A model drawn as an item's picture or a category's icon (`pictures.ts`). */
export type Picture =
  | { kind: 'road'; roadClass: RoadClass }
  | { kind: 'zone'; zone: Zone }
  | { kind: 'plant'; plant: PlantKind }
  | { kind: 'line' }
  | { kind: 'icon'; name: 'roads' | 'zones' | 'power' | 'bulldoze' | 'views' | 'eraser' };

/** A stable name for a picture model, so each is drawn once. */
export function pictureKey(p: Picture): string {
  return Object.values(p).join(':');
}

export interface Item {
  /** Unique across the list; also the button's element ID. */
  id: string;
  label: string;
  /** One line for the tooltip. */
  tip: string;
  /** The shortcut as shown, such as `Z` or `Z twice`. */
  key?: string;
  picture: Picture;
  tool: Tool;
  /** CSS colour of the strip under the card: the zone's overlay colour. */
  stripe?: string;
}

export interface Option {
  id: string;
  label: string;
  tip: string;
  key?: string;
  action: OptionAction;
  /** Options sharing a group sit behind one button that opens them, such as Snaps. */
  group?: string;
}

export interface Category {
  id: string;
  label: string;
  tip: string;
  icon: Picture;
  /** A category with no items is not shown. */
  items: Item[];
  options?: Option[];
}

const css = (hex: number) => `#${hex.toString(16).padStart(6, '0')}`;
const zone = (id: string, label: string, z: Zone, key: string, tip: string): Item => ({
  id,
  label,
  tip,
  key,
  picture: z === Zone.None ? { kind: 'icon', name: 'eraser' } : { kind: 'zone', zone: z },
  tool: { kind: 'zone', zone: z },
  stripe: css(ZONE_OVERLAY[z] ?? 0x9aa6b4),
});
const plant = (id: string, label: string, kind: PlantKind, key: string, tip: string): Item => ({
  id,
  label,
  tip,
  key,
  picture: { kind: 'plant', plant: kind },
  tool: { kind: 'plant', plant: kind },
});

export const CATEGORIES: Category[] = [
  {
    id: 'roads',
    label: 'Roads',
    tip: 'Streets and main roads',
    icon: { kind: 'icon', name: 'roads' },
    items: [
      {
        id: 'street',
        label: 'Street',
        tip: 'Two lanes for homes and shops. R starts a straight road.',
        picture: { kind: 'road', roadClass: RoadClass.Street },
        tool: { kind: 'road', roadClass: RoadClass.Street },
      },
      {
        id: 'main-road',
        label: 'Main road',
        tip: 'Faster and wider; carries long trips.',
        key: 'M',
        picture: { kind: 'road', roadClass: RoadClass.Main },
        tool: { kind: 'road', roadClass: RoadClass.Main },
      },
    ],
    options: [
      {
        id: 'straight',
        label: 'Straight',
        tip: 'Click the start, then the end.',
        key: 'R',
        action: { kind: 'roadMode', mode: 'straight' },
      },
      {
        id: 'curve',
        label: 'Curve',
        tip: 'Click the start, a bend point, then the end.',
        key: 'V',
        action: { kind: 'roadMode', mode: 'curve' },
      },
      {
        id: 'continuous-curve',
        label: 'Continuous',
        tip: 'A chain of smoothly joined curves.',
        action: { kind: 'roadMode', mode: 'continuous' },
      },
      {
        id: 'snap-angle',
        label: 'Angle',
        tip: 'Snap to 15° steps from the road you start on.',
        group: 'Snaps',
        action: { kind: 'snap', flag: Snap.Angle },
      },
      {
        id: 'snap-guides',
        label: 'Guide lines',
        tip: 'Line up with nearby road ends.',
        group: 'Snaps',
        action: { kind: 'snap', flag: Snap.Guides },
      },
      {
        id: 'snap-length',
        label: 'Length',
        tip: 'Snap lengths to whole 8 m cells.',
        group: 'Snaps',
        action: { kind: 'snap', flag: Snap.Length },
      },
    ],
  },
  {
    id: 'zones',
    label: 'Zones',
    tip: 'Paint where homes, shops and industry may grow',
    icon: { kind: 'icon', name: 'zones' },
    items: [
      zone('houses', 'Houses', Zone.ResidentialLow, 'Z', 'Low-density homes.'),
      zone('apartments', 'Apartments', Zone.ResidentialHigh, 'Z twice', 'Dense homes.'),
      zone('small-shops', 'Small shops', Zone.CommercialLow, 'X', 'Corner shops.'),
      zone('stores', 'Stores', Zone.CommercialHigh, 'X twice', 'Big stores.'),
      zone('light-industry', 'Light industry', Zone.IndustrialLow, 'C', 'Workshops.'),
      zone('heavy-industry', 'Heavy industry', Zone.IndustrialHigh, 'C twice', 'Factories.'),
      zone('unzone', 'Unzone', Zone.None, 'U', 'Remove zoning.'),
    ],
  },
  {
    id: 'power',
    label: 'Power',
    tip: 'Plants and power lines',
    icon: { kind: 'icon', name: 'power' },
    items: [
      plant('plant-coal', 'Coal plant', PlantKind.Coal, 'G', 'Steady power; stands beside a road.'),
      plant('plant-solar', 'Solar farm', PlantKind.Solar, 'G twice', 'Power by day only.'),
      plant('plant-wind', 'Wind turbine', PlantKind.Wind, 'G three times', 'Power with the wind.'),
      {
        id: 'power-line',
        label: 'Power line',
        tip: 'Joins a plant to a road, or one road network to another.',
        key: 'L',
        picture: { kind: 'line' },
        tool: { kind: 'line' },
      },
    ],
  },
  // Later milestones fill these; until then they are hidden.
  {
    id: 'services',
    label: 'Services',
    tip: 'Police, fire, health and schools',
    icon: { kind: 'icon', name: 'zones' },
    items: [],
  },
  {
    id: 'transit',
    label: 'Transit',
    tip: 'Buses and trains',
    icon: { kind: 'icon', name: 'roads' },
    items: [],
  },
  {
    id: 'councilors',
    label: 'Councilors',
    tip: 'Helpers who build districts for you',
    icon: { kind: 'icon', name: 'zones' },
    items: [],
  },
];

/** Sits apart on the right of the category bar; it has no items of its own. */
export const BULLDOZE: Item = {
  id: 'bulldoze',
  label: 'Bulldoze',
  tip: 'Remove a road, building, plant or power line.',
  key: 'B',
  picture: { kind: 'icon', name: 'bulldoze' },
  tool: { kind: 'bulldoze' },
};
