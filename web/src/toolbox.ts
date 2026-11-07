// Which tool is in use: a road tool, bulldoze, a zone brush or a power tool, never two at once.
// The build menu and the keys both start tools here (M4.6); each tool keeps its own canvas
// handling and sim commands. Picking a power tool shows the power view until it is put away.
import { PowerTools } from './power-tools';
import { RoadClass, type ToWorker, Zone } from './protocol';
import type { Renderer } from './renderer';
import { RoadTool } from './road-tool';
import type { Selection } from './ui/build-menu';
import type { OptionAction, RoadMode, Tool } from './ui/catalog';
import { zoneHelpText } from './ui/hints';
import type { Views } from './ui/views';
import { ZoneTool } from './zone-tool';

export class Toolbox {
  readonly roads: RoadTool;
  readonly zoning: ZoneTool;
  readonly power: PowerTools;
  /** The road shape the road items start with: the last one picked. */
  private shape: RoadMode = 'straight';
  /** Whether the power view showed before a power tool turned it on; null when it did not. */
  private powerViewBefore: boolean | null = null;
  private readonly renderer: Renderer;
  private readonly hint: HTMLElement;
  private readonly views: Views;
  private readonly changed: () => void;

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
    // The player's own choice stands when the tool is put away.
    views.onPlayerPower = () => {
      this.powerViewBefore = null;
    };
  }

  get active() {
    return this.roads.mode !== null || this.zoning.zone !== null || this.power.active;
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
            : p !== null
              ? { kind: 'plant', plant: p }
              : null;
    const roadMode = roads.roadMode ? this.shape : null;
    return { tool, roadMode, snap: roads.snaps, gridCell: roads.cell };
  }

  /** Puts every tool away, then starts `tool`, if any. */
  use(tool: Tool | null) {
    if (tool?.kind === 'road' && this.roads.roadMode) {
      // Street and main road are a setting of the road tool; a road being drawn carries on.
      this.roads.setRoadClass(tool.roadClass);
      return this.changed();
    }
    // The hint line is shared, and each tool clears only what it wrote: stop them all first.
    this.power.set(null);
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
    this.showPowerView(this.power.active);
    this.renderer.renderer.domElement.style.cursor = tool ? 'crosshair' : '';
    this.changed();
  }

  /** Esc: puts the tool in use away; false if none was. */
  cancel(): boolean {
    if (!this.active) return false;
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

  /** The tool keys: R, V, B, M, Z, X, C, U, G and L. False for any other key. */
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

  private showPowerView(on: boolean) {
    if (on && this.powerViewBefore === null) {
      this.powerViewBefore = this.views.power;
      this.views.setPower(true);
    } else if (!on && this.powerViewBefore !== null) {
      this.views.setPower(this.powerViewBefore);
      this.powerViewBefore = null;
    }
  }
}
