// The plant and transmission line tools and the power view (ADR 0041). Like the road tools,
// they ask the sim what a click would do and draw its answer; they never decide it themselves.
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
  setPowerView(on: boolean): void;
  setPower(plants: Float32Array, lines: Float32Array, buildings: Float32Array): void;
  renderer: { domElement: HTMLElement };
}

export class PowerTools {
  /** The plant being placed, `'line'`, or null when neither tool is on. */
  tool: PlantKind | 'line' | null = null;
  private view = false;
  private start: Spot | null = null;
  private at: Spot | null = null;
  private wanted = false;
  private inFlight = false;
  private revision = 0;
  /** The sim's latest line answer, so a click can chain from the snapped end. */
  private ends: Float32Array | null = null;
  private refused = false;

  private readonly help: HTMLElement;
  private readonly plantButtons: HTMLElement[];
  private readonly lineButton: HTMLElement;
  private readonly viewButton: HTMLElement;

  /** `stopOthers` switches every other tool off before a power tool starts. */
  private readonly send: (msg: ToWorker) => void;
  private readonly draw: PowerView;
  private readonly stopOthers: () => void;

  constructor(
    send: (msg: ToWorker) => void,
    draw: PowerView,
    byId: (id: string) => HTMLElement,
    stopOthers: () => void,
  ) {
    this.send = send;
    this.draw = draw;
    this.stopOthers = stopOthers;
    this.help = byId('road-help');
    this.plantButtons = ['plant-coal', 'plant-solar', 'plant-wind'].map(byId);
    this.lineButton = byId('power-line');
    this.viewButton = byId('power-view');
    this.plantButtons.forEach((b, kind) => {
      b.onclick = () => this.choose(this.tool === kind ? null : (kind as PlantKind));
    });
    this.lineButton.onclick = () => this.choose(this.tool === 'line' ? null : 'line');
    this.viewButton.onclick = () => this.toggleView();
  }

  get active() {
    return this.tool !== null;
  }

  /** Switches tool without touching the others; `choose` switches them off first. */
  set(tool: PlantKind | 'line' | null) {
    const was = this.tool;
    this.tool = tool;
    this.start = this.ends = null;
    this.refused = this.inFlight = false;
    this.clearPreview();
    this.plantButtons.forEach((b, kind) => {
      press(b, kind === tool);
    });
    press(this.lineButton, tool === 'line');
    if (tool !== null || was !== null) this.showHelp();
    if (tool !== null) this.draw.renderer.domElement.style.cursor = 'crosshair';
  }

  /** Starts a power tool, or stops every tool with null. */
  choose(tool: PlantKind | 'line' | null) {
    this.stopOthers();
    this.set(tool);
  }

  /** G cycles coal, solar, wind and off; L toggles the line tool; O the power view. */
  key(key: string): boolean {
    const k = key.toLowerCase();
    const t = this.tool;
    if (k === 'g')
      this.choose(
        typeof t !== 'number'
          ? PlantKind.Coal
          : t === PlantKind.Wind
            ? null
            : ((t + 1) as PlantKind),
      );
    else if (k === 'l') this.choose(t === 'line' ? null : 'line');
    else if (k === 'o') this.toggleView();
    else return false;
    return true;
  }

  /** A pointer release on the canvas while a power tool is on. */
  release(button: number, still: boolean, p: Spot | null) {
    if (button === 2) this.choose(null);
    else if (button === 0 && still && p) this.click(p);
  }

  private toggleView() {
    this.view = !this.view;
    press(this.viewButton, this.view);
    this.draw.setPowerView(this.view);
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
          ? `${no}${this.start ? 'Click the line end.' : 'Click where the power line starts: a plant, road or line.'} Esc or right-click stops.`
          : `${PLANT_NAMES[t]}: click to place. ${PLANT_HELP[t]} G switches plant, Esc stops.`;
  }
}

function press(b: HTMLElement, on: boolean) {
  b.classList.toggle('active', on);
  b.setAttribute('aria-pressed', String(on));
}
