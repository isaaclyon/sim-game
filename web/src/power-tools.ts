// The plant, transmission line, pump and water tower tools (ADR 0041, 0052). Like the road
// tools, they ask the sim what a click would do and draw its answer; they never decide it
// themselves. The build menu owns their buttons and the Views list owns the power and water
// views.
import { type FromWorker, PlantKind, type ToWorker, type WaterKind } from './protocol';

const PLANT_NAMES = ['Coal plant', 'Solar farm', 'Wind turbine'];
const PLANT_HELP = [
  'Coal must stand beside a road.',
  'Join it to a road with a power line (L).',
  'Join it to a road with a power line (L).',
];
/** Water tools by `WaterKind`. */
export const WATER_TOOLS = ['pump', 'tower'] as const;
export type WaterTool = (typeof WATER_TOOLS)[number];
const WATER_HELP: Record<WaterTool, string> = {
  pump: 'Pump: click beside a road. It gives less water on polluted ground.',
  tower: 'Water tower: click beside a road. It stores spare water for shortages.',
};

/** A placement tool: a plant kind, a power line, or a water piece. */
export type PlacementTool = PlantKind | 'line' | WaterTool;

function isWater(t: PlacementTool | null): t is WaterTool {
  return t === 'pump' || t === 'tower';
}

const waterKind = (t: WaterTool) => WATER_TOOLS.indexOf(t) as WaterKind;

/** A ground point in sim coordinates: x, and z for the sim's y. */
export interface Spot {
  x: number;
  z: number;
}

/** What the tools draw on; `Renderer` implements it. */
export interface PowerView {
  setPlantPreview(outline: Float32Array, ok: boolean): void;
  setLinePreview(ends: Float32Array | null, a: Spot | null, b: Spot | null): void;
  setPower(plants: Float32Array, lines: Float32Array, buildings: Float32Array): void;
  setWater(pieces: Float32Array, buildings: Float32Array): void;
}

export class PowerTools {
  /** The plant or water piece being placed, `'line'`, or null when no tool is on. */
  tool: PlacementTool | null = null;
  private start: Spot | null = null;
  private at: Spot | null = null;
  private wanted = false;
  private inFlight = false;
  private revision = 0;
  /** The sim's latest line answer, so a click can chain from the snapped end. */
  private ends: Float32Array | null = null;
  private refused = false;

  private readonly send: (msg: ToWorker) => void;
  private readonly draw: PowerView;
  /** The build menu's shared hint line. */
  private readonly help: HTMLElement;
  /** Puts every tool away, as a right-click does. */
  private readonly stop: () => void;

  constructor(send: (msg: ToWorker) => void, draw: PowerView, help: HTMLElement, stop: () => void) {
    this.send = send;
    this.draw = draw;
    this.help = help;
    this.stop = stop;
  }

  get active() {
    return this.tool !== null;
  }

  /** Switches tool without touching the others; `Toolbox.use` puts them away first. */
  set(tool: PlacementTool | null) {
    const was = this.tool;
    this.tool = tool;
    this.start = this.ends = null;
    this.refused = this.inFlight = false;
    this.clearPreview();
    if (tool !== null || was !== null) this.showHelp();
  }

  /** The tool G picks next: coal, solar, wind, then none. */
  nextPlant(): PlantKind | null {
    const t = this.tool;
    return typeof t !== 'number'
      ? PlantKind.Coal
      : t === PlantKind.Wind
        ? null
        : ((t + 1) as PlantKind);
  }

  /** A pointer release on the canvas while a power tool is on. */
  release(button: number, still: boolean, p: Spot | null) {
    if (button === 2) this.stop();
    else if (button === 0 && still && p) this.click(p);
  }

  private click(p: Spot) {
    const t = this.tool;
    if (t === null) return;
    if (isWater(t)) {
      this.send({ type: 'placeWater', kind: waterKind(t), x: p.x, y: p.z });
    } else if (t !== 'line') {
      this.send({ type: 'placePlant', kind: t, x: p.x, y: p.z });
    } else if (!this.start) {
      this.start = p;
    } else if (this.ends && this.ends.length === 6) {
      const a = this.start;
      this.send({ type: 'buildLine', ax: a.x, ay: a.z, bx: p.x, by: p.z });
      // Chain from where the sim put this end, as the road tool does.
      this.start = { x: this.ends[3], z: this.ends[4] };
    } else {
      this.refused = true;
    }
    this.ends = null;
    this.revision++;
    this.inFlight = false;
    this.showHelp();
    this.hover(p);
  }

  /** The cursor is over `p`, or off the ground. */
  hover(p: Spot | null) {
    this.at = p;
    this.wanted = true;
    this.ask();
  }

  /** Handles the power and water messages; true means `msg` was one of them. */
  receive(
    msg: FromWorker,
  ): msg is Extract<
    FromWorker,
    { type: 'power' | 'water' | 'plantPreview' | 'waterPreview' | 'linePreview' }
  > {
    if (msg.type === 'power') {
      this.draw.setPower(msg.plants, msg.lines, msg.buildings);
      return true;
    }
    if (msg.type === 'water') {
      this.draw.setWater(msg.pieces, msg.buildings);
      return true;
    }
    const placed = msg.type === 'plantPreview' || msg.type === 'waterPreview';
    if (!placed && msg.type !== 'linePreview') return false;
    this.inFlight = false;
    if (msg.revision === this.revision && this.at) {
      const t = this.tool;
      const wanted = msg.type === 'plantPreview' ? typeof t === 'number' : isWater(t);
      if (placed && wanted) this.draw.setPlantPreview(msg.outline, msg.ok);
      if (msg.type === 'linePreview' && this.tool === 'line') {
        this.ends = msg.ends;
        this.draw.setLinePreview(msg.ends, this.start, this.at);
      }
    }
    this.ask();
    return true;
  }

  private ask() {
    const [t, p, a] = [this.tool, this.at, this.start];
    if (!this.wanted || this.inFlight || t === null) return;
    if (!p) return this.clearPreview();
    this.wanted = false;
    if (t === 'line' && !a) {
      this.draw.setLinePreview(null, null, null);
      return;
    }
    this.inFlight = true;
    const revision = this.revision;
    const at = { x: p.x, y: p.z, revision };
    this.send(
      t === 'line' && a
        ? { type: 'previewLine', ax: a.x, ay: a.z, bx: p.x, by: p.z, revision }
        : isWater(t)
          ? { type: 'previewWater', kind: waterKind(t), ...at }
          : { type: 'previewPlant', kind: t as PlantKind, ...at },
    );
  }

  private clearPreview() {
    this.revision++;
    this.wanted = false;
    this.draw.setPlantPreview(new Float32Array(), true);
    this.draw.setLinePreview(null, null, null);
  }

  private showHelp() {
    const t = this.tool;
    const no = this.refused ? 'Cannot build a line there. ' : '';
    this.help.textContent =
      t === null
        ? ''
        : t === 'line'
          ? `${no}${this.start ? 'Click the line end.' : 'Click where the line starts: a plant, road or line.'} Esc stops.`
          : isWater(t)
            ? `${WATER_HELP[t]} Esc stops.`
            : `${PLANT_NAMES[t]}: click to place. ${PLANT_HELP[t]} Esc stops.`;
  }
}
