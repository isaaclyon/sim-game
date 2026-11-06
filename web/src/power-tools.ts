// The plant and transmission line tools (ADR 0041). Like the road tools, they ask the sim what
// a click would do and draw its answer; they never decide it themselves. The build menu owns
// their buttons and the Views list owns the power view.
import { type FromWorker, PlantKind, type ToWorker } from './protocol';

const PLANT_NAMES = ['Coal plant', 'Solar farm', 'Wind turbine'];
const PLANT_HELP = [
  'Coal must stand beside a road.',
  'Join it to a road with a power line (L).',
  'Join it to a road with a power line (L).',
];

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
}

export class PowerTools {
  /** The plant being placed, `'line'`, or null when neither tool is on. */
  tool: PlantKind | 'line' | null = null;
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
  set(tool: PlantKind | 'line' | null) {
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
    if (t !== 'line') {
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

  /** Handles the power messages; true means `msg` was one of them. */
  receive(
    msg: FromWorker,
  ): msg is Extract<FromWorker, { type: 'power' | 'plantPreview' | 'linePreview' }> {
    if (msg.type === 'power') {
      this.draw.setPower(msg.plants, msg.lines, msg.buildings);
      return true;
    }
    if (msg.type !== 'plantPreview' && msg.type !== 'linePreview') return false;
    this.inFlight = false;
    if (msg.revision === this.revision && this.at) {
      if (msg.type === 'plantPreview' && typeof this.tool === 'number') {
        this.draw.setPlantPreview(msg.outline, msg.ok);
      }
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
    this.send(
      t === 'line' && a
        ? { type: 'previewLine', ax: a.x, ay: a.z, bx: p.x, by: p.z, revision }
        : { type: 'previewPlant', kind: t as PlantKind, x: p.x, y: p.z, revision },
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
          : `${PLANT_NAMES[t]}: click to place. ${PLANT_HELP[t]} Esc stops.`;
  }
}
