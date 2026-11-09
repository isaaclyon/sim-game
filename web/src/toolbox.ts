// Which tool is in use: a road tool, bulldoze, a zone brush, a power tool or the fire station
// tool, never two at once.
// The build menu and the keys both start tools here (M4.6); each tool keeps its own canvas
// handling and sim commands. Picking a power, water or fire station tool shows that service's
// view until it is put away. Tools the city's era has not unlocked are refused with a hint
// (ADR 0056).
import { FireTools } from './fire-tools';
import { PowerTools, WATER_TOOLS } from './power-tools';
import { Era, type Prices, RoadClass, type ToWorker, type WaterKind, Zone } from './protocol';
import type { Renderer } from './renderer';
import { RoadTool } from './road-tool';
import { type Selection, sameTool } from './ui/build-menu';
import { CATEGORIES, type OptionAction, type RoadMode, type Tool, unlockEra } from './ui/catalog';
import { lockedText, zoneHelpText } from './ui/hints';
import type { Service, Views } from './ui/views';
import { ZoneTool } from './zone-tool';

export class Toolbox {
  readonly roads: RoadTool;
  readonly zoning: ZoneTool;
  readonly power: PowerTools;
  readonly fire: FireTools;
  /** The road shape the road items start with: the last one picked. */
  private shape: RoadMode = 'straight';
  /** Whether each service's view showed before its tool turned it on; null when it did not. */
  private viewBefore: Record<Service, boolean | null> = { power: null, water: null, fire: null };
  private readonly renderer: Renderer;
  private readonly hint: HTMLElement;
  private readonly views: Views;
  private readonly changed: () => void;
  /** The city's era, and the residents still needed for the next; town until the sim says. */
  private era: Era = Era.Town;
  private needed = 0;
  /** Whether the hint says why a locked tool was refused. */
  private lockHint = false;

  /** `changed` runs after every switch, so the menu can show what is in use. */
  constructor(
    send: (msg: ToWorker) => void,
    renderer: Renderer,
    el: { hint: HTMLElement; readout: HTMLElement },
    views: Views,
    changed: () => void,
  ) {
    this.renderer = renderer;
    this.hint = el.hint;
    this.views = views;
    this.changed = changed;
    this.roads = new RoadTool(send, renderer, { help: el.hint, readout: el.readout });
    this.zoning = new ZoneTool(send, renderer);
    this.power = new PowerTools(send, renderer, el.hint, () => this.use(null));
    this.fire = new FireTools(send, renderer, el.hint, () => this.use(null));
    // The player's own choice stands when the tool is put away.
    views.onPlayerMarkers = (service) => {
      this.viewBefore[service] = null;
    };
  }

  get active() {
    const { roads, zoning, power, fire } = this;
    return roads.mode !== null || zoning.zone !== null || power.active || fire.active;
  }

  get selection(): Selection {
    const { roads, zoning, power } = this;
    const p = power.tool;
    const tool: Tool | null = roads.roadMode
      ? { kind: 'road', roadClass: roads.roadClass }
      : roads.mode === 'bulldoze'
        ? { kind: 'bulldoze' }
        : zoning.zone !== null
          ? { kind: 'zone', zone: zoning.zone }
          : p === 'line'
            ? { kind: 'line' }
            : p === 'pump' || p === 'tower'
              ? { kind: 'water', water: WATER_TOOLS.indexOf(p) as WaterKind }
              : p !== null
                ? { kind: 'plant', plant: p }
                : this.fire.active
                  ? { kind: 'station' }
                  : null;
    const roadMode = roads.roadMode ? this.shape : null;
    return { tool, roadMode, snap: roads.snaps, gridCell: roads.cell };
  }

  /** Whether the city's era has not unlocked `tool` yet. */
  locked(tool: Tool | null): boolean {
    return unlockEra(tool) > this.era;
  }

  /** The city's era from the sim's stats. A tool it locks, such as after loading a village,
   * is put away. */
  setEra(era: Era, needed: number) {
    this.needed = needed;
    if (era === this.era) return;
    this.era = era;
    // A refused tool's hint is out of date once the era changes.
    if (this.lockHint) {
      this.lockHint = false;
      this.hint.textContent = '';
      this.changed();
    }
    if (this.locked({ kind: 'road', roadClass: this.roads.roadClass })) {
      if (this.roads.roadMode) this.use(null);
      this.roads.setRoadClass(RoadClass.Street);
    }
    if (this.locked(this.selection.tool)) this.use(null);
  }

  /** Puts every tool away, then starts `tool`, if any. A locked tool changes nothing but the
   * hint, which says what it waits for. */
  use(tool: Tool | null) {
    if (this.locked(tool)) {
      const item = CATEGORIES.flatMap((c) => c.items).find((i) => sameTool(i.tool, tool));
      this.hint.textContent = `${item?.label ?? 'This tool'}. ${lockedText(this.needed)}`;
      this.lockHint = true;
      return this.changed();
    }
    this.lockHint = false;
    if (tool?.kind === 'road' && this.roads.roadMode) {
      // Street and main road are a setting of the road tool; a road being drawn carries on.
      this.roads.setRoadClass(tool.roadClass);
      return this.changed();
    }
    // The hint line is shared, and each tool clears only what it wrote: stop them all first.
    this.power.set(null);
    this.fire.set(false);
    this.zoning.set(null);
    this.roads.setMode(null);
    this.hint.textContent = '';
    if (tool?.kind === 'road') {
      this.roads.setRoadClass(tool.roadClass);
      this.roads.setMode(this.shape);
    } else if (tool?.kind === 'bulldoze') this.roads.setMode('bulldoze');
    else if (tool?.kind === 'zone') {
      this.zoning.set(tool.zone);
      this.hint.textContent = zoneHelpText(tool.zone);
    } else if (tool?.kind === 'plant') this.power.set(tool.plant);
    else if (tool?.kind === 'line') this.power.set('line');
    else if (tool?.kind === 'water') this.power.set(WATER_TOOLS[tool.water]);
    else if (tool?.kind === 'station') this.fire.set(true);
    const t = this.power.tool;
    const water = t === 'pump' || t === 'tower';
    this.showView('power', t !== null && !water);
    this.showView('water', water);
    this.showView('fire', this.fire.active);
    this.renderer.renderer.domElement.style.cursor = tool ? 'crosshair' : '';
    this.changed();
  }

  /** The sim's prices, for the placement tools' hints. */
  setPrices(prices: Prices) {
    this.power.prices = this.fire.prices = prices;
  }

  /** Undo or redo changed the map under a road or line being drawn: start it again. */
  dropDrafts() {
    if (this.roads.mode) this.roads.setMode(this.roads.mode);
    if (this.power.tool) this.power.set(this.power.tool);
  }

  /** Esc: puts the tool in use away; false if none was. */
  cancel(): boolean {
    if (!this.active) {
      if (!this.hint.textContent) return false;
      // A locked tool's hint shows on its own; Esc dismisses it.
      this.hint.textContent = '';
      this.lockHint = false;
      this.changed();
      return true;
    }
    this.use(null);
    return true;
  }

  /** An option in the build menu's options row. A block size starts the grid tool. */
  option(action: OptionAction) {
    if (action.kind === 'roadMode') this.setShape(action.mode);
    else if (action.kind === 'gridCell') {
      this.roads.setCell(action.cell);
      this.setShape('grid');
    } else {
      this.roads.toggleSnap(action.flag);
      this.changed();
    }
  }

  /** The tool keys: R, V, B, M, Z, X, C, U, G, L and H. False for any other key. */
  key(key: string): boolean {
    const k = key.toLowerCase();
    const toggle = (mode: RoadMode) =>
      this.roads.mode === mode ? this.use(null) : this.setShape(mode);
    if (k === 'r') toggle('straight');
    else if (k === 'v') toggle('curve');
    else if (k === 'b') this.use(this.roads.mode === 'bulldoze' ? null : { kind: 'bulldoze' });
    else if (k === 'm') this.toggleMainRoad();
    else if (k === 'z') this.zoneKey(Zone.ResidentialLow, Zone.ResidentialHigh);
    else if (k === 'x') this.zoneKey(Zone.CommercialLow, Zone.CommercialHigh);
    else if (k === 'c') this.zoneKey(Zone.IndustrialLow, Zone.IndustrialHigh);
    else if (k === 'u') this.useZone(this.zoning.zone === Zone.None ? null : Zone.None);
    else if (k === 'g') {
      const plant = this.power.nextPlant();
      this.use(plant === null ? null : { kind: 'plant', plant });
    } else if (k === 'l') this.use(this.power.tool === 'line' ? null : { kind: 'line' });
    else if (k === 'h') this.use(this.fire.active ? null : { kind: 'station' });
    else return false;
    return true;
  }

  /** Starts the road tool in `mode`, or switches the one in use to it. */
  private setShape(mode: RoadMode) {
    this.shape = mode;
    if (this.roads.roadMode) {
      this.roads.setMode(mode);
      this.changed();
    } else this.use({ kind: 'road', roadClass: this.roads.roadClass });
  }

  /** M switches street and main road, picking the straight road tool if no road tool is on. */
  private toggleMainRoad() {
    const roadClass = this.roads.roadClass === RoadClass.Main ? RoadClass.Street : RoadClass.Main;
    if (this.roads.roadMode) return this.use({ kind: 'road', roadClass });
    this.shape = 'straight';
    this.use({ kind: 'road', roadClass });
  }

  /** Z, X and C pick a zone; pressing again switches density. */
  private zoneKey(low: Zone, high: Zone) {
    this.useZone(this.zoning.zone === low ? high : low);
  }

  private useZone(zone: Zone | null) {
    this.use(zone === null ? null : { kind: 'zone', zone });
  }

  private showView(service: Service, on: boolean) {
    const before = this.viewBefore[service];
    if (on && before === null) {
      this.viewBefore[service] = this.views.markers[service];
      this.views.setMarkers(service, true);
    } else if (!on && before !== null) {
      this.views.setMarkers(service, before);
      this.viewBefore[service] = null;
    }
  }
}
