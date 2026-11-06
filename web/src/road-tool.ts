// The road, upgrade and bulldoze tools: clicked points, sim-owned previews and drawing guides
// (ADR 0038). Upgrade clicks a road to give it the chosen class (ADR 0050).
// The sim guides and plans every point; this module only keeps the clicks, asks one question at
// a time about the latest cursor position, and draws the answers.

import { type FromWorker, RoadClass, Snap, type ToWorker } from './protocol';
import { type Quote, roadHelpText } from './ui/hints';

export type RoadMode = 'straight' | 'curve' | 'continuous' | 'upgrade' | 'bulldoze' | null;
/** A ground position: x and z in world space, sim x and y. */
type Ground = { x: number; z: number };
/** A clicked point with the snap flags it was clicked with. */
type Clicked = Ground & { snap: number };

interface Drawing {
  setPreview(stops: Float32Array | null, a: Ground | null, b: Ground | null, cls?: number): void;
  setGuides(lines: Float32Array | null): void;
  setAngles(marks: Float32Array | null): void;
  setBulldozePreview(segment: Float32Array): void;
  setUpgradePreview(segment: Float32Array, ok: boolean, cls: number): void;
}

interface Elements {
  /** The build menu's hint line, shared by every tool. */
  help: HTMLElement;
  readout: HTMLElement;
}

const SNAP_ALL = Snap.Angle | Snap.Guides | Snap.Length;
const READOUT_OFFSET_PX = 16;
/** Closer than this to the right or bottom edge, the readout flips to the cursor's other side. */
const READOUT_EDGE_PX = 120;

export class RoadTool {
  mode: RoadMode = null;
  roadClass: RoadClass = RoadClass.Street;
  private clicked: Clicked[] = [];
  private previousControl: Ground | null = null;
  private refused = false;
  private nextBuildId = 0;
  private pendingBuild: number | null = null;
  /** The sim's price for the previewed road, and for the build in flight (ADR 0036). */
  private quote: Quote = null;
  private pendingQuote: Quote = null;
  private snap = SNAP_ALL;
  private shift = false;
  // One sim-owned preview in flight, using only the latest cursor position.
  private revision = 0;
  private cursor: Ground | null = null;
  private screen: { clientX: number; clientY: number } | null = null;
  private wanted = false;
  private inFlight = false;
  /** Whether the question in flight plans a road, or only guides a curve's control point. */
  private planAsked = true;

  private send: (msg: ToWorker) => void;
  private draw: Drawing;
  private el: Elements;

  constructor(send: (msg: ToWorker) => void, draw: Drawing, el: Elements) {
    this.send = send;
    this.draw = draw;
    this.el = el;
  }

  /** Turns one drawing guide (`Snap` flag) on or off; the preview follows at once. */
  toggleSnap(flag: number) {
    this.snap ^= flag;
    this.ask();
  }

  /** The drawing guides that are on, as `Snap` flags. */
  get snaps() {
    return this.snap;
  }

  /** A tool that uses the road class: drawing roads, or upgrading them. */
  get roadMode() {
    return this.mode !== null && this.mode !== 'bulldoze';
  }

  /** Switches tool, dropping any road being drawn. */
  setMode(mode: RoadMode) {
    const was = this.mode;
    this.mode = mode;
    this.clicked = [];
    this.previousControl = null;
    this.pendingBuild = null;
    this.refused = false;
    this.invalidate();
    // The hint line is shared: clear it only if this tool was the one using it.
    if (mode) this.updateHelp();
    else if (was) this.el.help.textContent = '';
  }

  /** A fresh city: nothing is in flight any more. */
  reset() {
    this.inFlight = false;
    this.setMode(null);
  }

  setRoadClass(cls: RoadClass) {
    this.roadClass = cls;
    // The price depends on the class: ask again for the same cursor.
    this.quote = null;
    this.roadsChanged();
    this.updateHelp();
  }

  /** Holding Shift places points freely; the preview follows at once. */
  setShift(on: boolean) {
    if (on === this.shift) return;
    this.shift = on;
    this.ask();
  }

  hover(p: Ground | null, screen: { clientX: number; clientY: number }) {
    if (!this.mode || this.pendingBuild !== null) return;
    // Keep the latest completed preview visible during movement. Only a change of tool or
    // construction stage invalidates an in-flight response; cursor updates queue the next one.
    this.cursor = p;
    this.screen = screen;
    this.ask();
  }

  /** The cursor left the canvas. */
  leave() {
    this.invalidate();
  }

  click(p: Ground, shift: boolean) {
    if (this.pendingBuild !== null || !this.mode) return;
    this.shift = shift;
    if (this.mode === 'bulldoze') this.send({ type: 'deleteRoad', x: p.x, y: p.z });
    else if (this.mode === 'upgrade')
      this.send({ type: 'setRoadClass', x: p.x, y: p.z, class: this.roadClass });
    else if (this.clicked.length < (this.needsControl() ? 2 : 1))
      this.clicked.push({ ...p, snap: this.flags() });
    else {
      const id = ++this.nextBuildId;
      this.pendingBuild = id;
      this.pendingQuote = this.quote;
      this.send({
        type: 'buildRoad',
        id,
        points: this.points(p),
        class: this.roadClass,
        ...(this.mode === 'continuous' ? { chain: true } : {}),
      });
    }
    this.refused = false;
    this.invalidate();
    this.updateHelp();
  }

  /** Roads changed: ask again about the cursor, which may now be over something else. */
  roadsChanged() {
    if (!this.cursor) return;
    this.revision++;
    this.ask();
  }

  receive(msg: FromWorker) {
    if (
      msg.type === 'roadPreview' ||
      msg.type === 'bulldozePreview' ||
      msg.type === 'roadClassPreview'
    ) {
      this.inFlight = false;
      if (msg.revision === this.revision && this.cursor) {
        if (msg.type === 'roadPreview' && this.drawing) this.showRoad(msg);
        if (msg.type === 'bulldozePreview' && this.mode === 'bulldoze')
          this.draw.setBulldozePreview(msg.segment);
        if (msg.type === 'roadClassPreview' && this.mode === 'upgrade') this.showUpgrade(msg);
      }
      this.sendPreview();
    } else if (msg.type === 'buildResult') {
      if (this.pendingBuild !== msg.id) return;
      // The guided end is already on the network, so it keeps its place without snapping.
      if (msg.end) {
        this.previousControl = msg.control ? { x: msg.control[0], z: msg.control[1] } : null;
        this.clicked =
          this.mode === 'continuous' && !this.previousControl
            ? []
            : [
                {
                  x: msg.end[0],
                  z: msg.end[1],
                  snap: this.mode === 'continuous' ? Snap.Continue : 0,
                },
              ];
      }
      // A road the city could not pay for was refused for money, not for its shape.
      const q = this.pendingQuote;
      const unpaid = !msg.end && q?.affordable === false ? q : null;
      this.pendingBuild = null;
      this.refused = !msg.end && !unpaid;
      this.invalidate();
      this.quote = unpaid;
      this.updateHelp();
    }
  }

  /** The road a click would change, at its new class's width, and its price. */
  private showUpgrade(msg: Extract<FromWorker, { type: 'roadClassPreview' }>) {
    this.draw.setUpgradePreview(msg.segment, msg.affordable, this.roadClass);
    this.quote = msg.segment.length > 0 ? { cost: msg.cost, affordable: msg.affordable } : null;
    this.updateHelp();
  }

  /** A tool that draws roads from clicked points. */
  private get drawing() {
    return this.roadMode && this.mode !== 'upgrade';
  }

  private showRoad(msg: Extract<FromWorker, { type: 'roadPreview' }>) {
    const pts = msg.points;
    const at = (i: number): Ground => ({ x: pts[2 * i], z: pts[2 * i + 1] });
    const n = pts.length / 2;
    if (n === 0) {
      this.draw.setPreview(null, null, null);
      this.draw.setGuides(null);
      this.draw.setAngles(null);
      this.refused = true;
      this.quote = null;
      this.el.readout.hidden = true;
      this.updateHelp();
      return;
    }
    if (!this.planAsked && n === 2) {
      // A curve's control guide, not a road. Only the sim evaluates the actual Bézier curve.
      this.draw.setPreview(new Float32Array([pts[0], pts[1], 0, pts[2], pts[3], -1]), at(0), at(1));
    } else this.draw.setPreview(msg.stops, n > 1 ? at(0) : null, at(n - 1), this.roadClass);
    this.draw.setGuides(msg.guides);
    this.draw.setAngles(msg.angles);
    this.refused = this.planAsked && n > 1 && msg.stops.length === 0;
    this.quote =
      this.planAsked && n > 1 && !this.refused
        ? { cost: msg.cost, affordable: msg.affordable }
        : null;
    const show = this.planAsked && n > 1 && this.screen !== null;
    this.el.readout.hidden = !show;
    if (show && this.screen) {
      const angle = Number.isNaN(msg.angle) ? '' : ` · ${Math.round(msg.angle)}°`;
      this.el.readout.textContent = `${Math.round(msg.length)} m${angle}`;
      const { clientX: x, clientY: y } = this.screen;
      // Near the right or bottom edge, the label goes on the other side of the cursor.
      const flipX = x > innerWidth - READOUT_EDGE_PX;
      const flipY = y > innerHeight - READOUT_EDGE_PX;
      const r = this.el.readout.style;
      r.left = `${x + (flipX ? -READOUT_OFFSET_PX : READOUT_OFFSET_PX)}px`;
      r.top = `${y + (flipY ? -READOUT_OFFSET_PX : READOUT_OFFSET_PX)}px`;
      r.transform = `translate(${flipX ? '-100%' : '0'}, ${flipY ? '-100%' : '0'})`;
    }
    this.updateHelp();
  }

  private needsControl() {
    return (
      this.mode === 'curve' ||
      (this.mode === 'continuous' && this.clicked[0]?.snap !== Snap.Continue)
    );
  }

  private flags() {
    return this.shift ? 0 : this.snap;
  }

  /** `[x, y, snap]` per clicked point, then `end` with the current flags. */
  private points(end: Ground) {
    const points = [...this.clicked, { ...end, snap: this.flags() }].flatMap((p) => [
      p.x,
      p.z,
      p.snap,
    ]);
    if (this.mode === 'continuous' && this.previousControl)
      points.push(this.previousControl.x, this.previousControl.z, 0);
    return points;
  }

  private ask() {
    this.wanted = true;
    this.sendPreview();
  }

  private sendPreview() {
    if (!this.wanted || !this.mode || !this.cursor || this.pendingBuild !== null) return;
    if (this.inFlight) return;
    this.wanted = false;
    this.inFlight = true;
    const revision = this.revision;
    const b = this.cursor;
    if (this.mode === 'bulldoze') {
      this.send({ type: 'previewBulldoze', revision, x: b.x, y: b.z });
      return;
    }
    if (this.mode === 'upgrade') {
      this.send({ type: 'previewRoadClass', revision, x: b.x, y: b.z, class: this.roadClass });
      return;
    }
    this.planAsked = !(this.needsControl() && this.clicked.length === 1);
    this.send({
      type: 'previewRoad',
      revision,
      class: this.roadClass,
      points: this.points(b),
      plan: this.planAsked,
    });
  }

  private invalidate() {
    this.revision++;
    this.cursor = null;
    this.wanted = false;
    this.quote = null;
    this.draw.setPreview(null, null, null);
    this.draw.setGuides(null);
    this.draw.setAngles(null);
    this.el.readout.hidden = true;
  }

  private updateHelp() {
    if (!this.mode) return;
    const stage = !this.clicked.length
      ? 'start'
      : this.needsControl() && this.clicked.length === 1
        ? 'control'
        : 'end';
    const main = this.roadClass === RoadClass.Main;
    const building = this.pendingBuild !== null;
    this.el.help.textContent = roadHelpText(
      this.mode,
      stage,
      building,
      main,
      this.refused,
      this.quote,
    );
  }
}
