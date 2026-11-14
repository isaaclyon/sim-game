// The school tools and the schools message (ADR 0055). Like the fire station tool, they ask
// the sim where a school would stand and draw its answer; they never decide that themselves.
import type { Spot } from './power-tools';
import type { FromWorker, Prices, SchoolKind, ToWorker } from './protocol';
import { placeText } from './ui/hints';

const NAMES = ['Primary school', 'High school'];

/** What the tools draw on; `Renderer` implements it. */
export interface SchoolView {
  setPlantPreview(outline: Float32Array, ok: boolean): void;
  setSchools(schools: Float32Array): void;
}

export class SchoolTools {
  /** The school being placed; null while the tool is put away. */
  kind: SchoolKind | null = null;
  private at: Spot | null = null;
  private wanted = false;
  private inFlight = false;
  private revision = 0;
  /** The latest preview said the city cannot pay. */
  private short = false;
  /** The sim's prices, for the hint; null until the first city starts. */
  prices: Prices | null = null;
  private readonly send: (msg: ToWorker) => void;
  private readonly draw: SchoolView;
  private readonly help: HTMLElement;
  private readonly stop: () => void;

  constructor(
    send: (msg: ToWorker) => void,
    draw: SchoolView,
    help: HTMLElement,
    stop: () => void,
  ) {
    this.send = send;
    this.draw = draw;
    this.help = help;
    this.stop = stop;
  }

  get active() {
    return this.kind !== null;
  }

  /** Picks the school to place, or puts the tool away; `Toolbox.use` stops the others first. */
  set(kind: SchoolKind | null) {
    const was = this.active;
    this.kind = kind;
    this.inFlight = this.short = false;
    if (was || kind !== null) this.clearPreview();
    if (kind !== null) this.showHelp();
    else if (was) this.help.textContent = '';
  }

  /** A pointer release on the canvas while the tool is on. */
  release(button: number, still: boolean, p: Spot | null) {
    if (button === 2) this.stop();
    else if (button === 0 && still && p && this.kind !== null) {
      this.send({ type: 'placeSchool', kind: this.kind, x: p.x, y: p.z });
      this.revision++;
      this.inFlight = false;
      this.hover(p);
    }
  }

  /** The cursor is over `p`, or off the ground. */
  hover(p: Spot | null) {
    this.at = p;
    this.wanted = true;
    this.ask();
  }

  /** Handles the school messages; true means `msg` was one of them. */
  receive(msg: FromWorker): msg is Extract<FromWorker, { type: 'schools' | 'schoolPreview' }> {
    if (msg.type === 'schools') {
      this.draw.setSchools(msg.schools);
      return true;
    }
    if (msg.type !== 'schoolPreview') return false;
    this.inFlight = false;
    if (msg.revision === this.revision && this.at && this.active) {
      this.draw.setPlantPreview(msg.outline, msg.ok);
      if (this.short === msg.affordable) {
        this.short = !msg.affordable;
        this.showHelp();
      }
    }
    this.ask();
    return true;
  }

  private ask() {
    if (!this.wanted || this.inFlight || this.kind === null) return;
    const p = this.at;
    if (!p) return this.clearPreview();
    this.wanted = false;
    this.inFlight = true;
    this.send({ type: 'previewSchool', kind: this.kind, x: p.x, y: p.z, revision: this.revision });
  }

  private showHelp() {
    if (this.kind === null) return;
    const price = placeText(NAMES[this.kind], this.prices?.schools[this.kind], !this.short);
    this.help.textContent = `${price} Click beside a road to place. Esc stops.`;
  }

  private clearPreview() {
    this.revision++;
    this.wanted = false;
    this.draw.setPlantPreview(new Float32Array(), true);
  }
}
