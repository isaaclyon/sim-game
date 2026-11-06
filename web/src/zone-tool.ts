// Zoning interactions only: the sim owns block outlines and the cells painted by commands.
// The build menu owns the zone buttons.
import type { Vector3 } from 'three';
import type { FromWorker, ToWorker, Zone } from './protocol';

interface Drawing {
  setZoneDrag(a: Vector3 | null, b: Vector3 | null, zone: number): void;
  setBlockOutline(outline: Float32Array, zone: number): void;
  setLeftDragPans(pans: boolean): void;
  setZoningVisible(visible: boolean): void;
}

export class ZoneTool {
  private brush: Zone | null = null;
  private start: Vector3 | null = null;
  private cursor: Vector3 | null = null;
  private keyPanned = false;
  // One block question in flight, about the latest cursor position only.
  private wanted = false;
  private inFlight = false;
  private revision = 0;
  private readonly send: (msg: ToWorker) => void;
  private readonly draw: Drawing;

  constructor(send: (msg: ToWorker) => void, draw: Drawing) {
    this.send = send;
    this.draw = draw;
  }

  get zone() {
    return this.brush;
  }

  set(zone: Zone | null) {
    this.brush = zone;
    this.start = null;
    this.hideOutline();
    this.draw.setLeftDragPans(zone === null);
    this.draw.setZoningVisible(zone !== null);
  }

  /** City replacement also releases the old city's pending request. */
  reset() {
    this.inFlight = false;
    this.set(null);
  }

  press(point: Vector3 | null) {
    this.start = point;
    this.keyPanned = false;
    this.revision++;
  }

  release(button: number, still: boolean, end: Vector3 | null) {
    const zone = this.brush;
    if (button === 0 && this.start && end && zone !== null) {
      const a = this.start;
      // A click fills a block; pointer or keyboard-camera movement paints a rectangle.
      if (!this.keyPanned && still) this.send({ type: 'fillBlock', x: end.x, y: end.z, zone });
      else this.send({ type: 'paintZone', ax: a.x, ay: a.z, bx: end.x, by: end.z, zone });
    }
    this.start = null;
    this.draw.setZoneDrag(null, null, 0);
    // The outline returns on the next move or when the edit's roads arrive.
    this.cursor = end;
    this.wanted = true;
  }

  hover(point: Vector3 | null) {
    if (this.brush === null) return;
    if (this.start) this.draw.setZoneDrag(this.start, point, this.brush);
    else {
      this.cursor = point;
      this.wanted = true;
      this.ask();
    }
  }

  leave() {
    if (!this.start) this.hideOutline();
  }

  cameraMoved() {
    if (this.start) this.keyPanned = true;
  }

  /** Road edits can change blocks, including alleys added after zoning. */
  roadsChanged() {
    if (!this.cursor) return;
    this.wanted = true;
    this.ask();
  }

  receive(msg: Extract<FromWorker, { type: 'blockPreview' }>) {
    this.inFlight = false;
    if (msg.revision === this.revision && this.brush !== null && !this.start && this.cursor)
      this.draw.setBlockOutline(msg.outline, this.brush);
    this.ask();
  }

  private ask() {
    if (!this.wanted || this.inFlight || !this.cursor || this.brush === null || this.start) return;
    this.wanted = false;
    this.inFlight = true;
    this.send({
      type: 'previewBlock',
      x: this.cursor.x,
      y: this.cursor.z,
      revision: this.revision,
    });
  }

  private hideOutline() {
    this.revision++;
    this.cursor = null;
    this.wanted = false;
    this.draw.setZoneDrag(null, null, 0);
  }
}
