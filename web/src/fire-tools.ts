// The fire station tool and the fire messages (ADR 0053). Like the plant tool, it asks the sim
// where a station would stand and draws its answer; it never decides that itself.
import type { Spot } from './power-tools';
import type { FromWorker, Prices, ToWorker } from './protocol';
import { placeText } from './ui/hints';

type FireMessage = Extract<FromWorker, { type: 'fire' }>;

/** What the tool draws on; `Renderer` implements it. */
export interface FireView {
  setPlantPreview(outline: Float32Array, ok: boolean): void;
  setFire(msg: FireMessage): void;
}

export class FireTools {
  /** The station tool is on. */
  active = false;
  private at: Spot | null = null;
  private wanted = false;
  private inFlight = false;
  private revision = 0;
  /** The latest preview said the city cannot pay. */
  private short = false;
  /** The sim's prices, for the hint; null until the first city starts. */
  prices: Prices | null = null;
  private readonly send: (msg: ToWorker) => void;
  private readonly draw: FireView;
  private readonly help: HTMLElement;
  private readonly stop: () => void;

  constructor(send: (msg: ToWorker) => void, draw: FireView, help: HTMLElement, stop: () => void) {
    this.send = send;
    this.draw = draw;
    this.help = help;
    this.stop = stop;
  }

  /** Switches the tool on or off; `Toolbox.use` puts the others away first. */
  set(on: boolean) {
    const was = this.active;
    this.active = on;
    this.inFlight = this.short = false;
    this.clearPreview();
    if (on) this.showHelp();
    else if (was) this.help.textContent = '';
  }

  /** A pointer release on the canvas while the tool is on. */
  release(button: number, still: boolean, p: Spot | null) {
    if (button === 2) this.stop();
    else if (button === 0 && still && p && this.active) {
      this.send({ type: 'placeStation', x: p.x, y: p.z });
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

  /** Handles the fire messages; true means `msg` was one of them. */
  receive(msg: FromWorker): msg is Extract<FromWorker, { type: 'fire' | 'stationPreview' }> {
    if (msg.type === 'fire') {
      this.draw.setFire(msg);
      return true;
    }
    if (msg.type !== 'stationPreview') return false;
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
    if (!this.wanted || this.inFlight || !this.active) return;
    const p = this.at;
    if (!p) return this.clearPreview();
    this.wanted = false;
    this.inFlight = true;
    this.send({ type: 'previewStation', x: p.x, y: p.z, revision: this.revision });
  }

  private showHelp() {
    const price = placeText('Fire station', this.prices?.station, !this.short);
    this.help.textContent = `${price} Click beside a road to place. Esc stops.`;
  }

  private clearPreview() {
    this.revision++;
    this.wanted = false;
    this.draw.setPlantPreview(new Float32Array(), true);
  }
}
