// Quiet, short engine pass-bys and birds, synthesised with Web Audio. No continuous noise loops
// or sound generated just because buildings exist. `ambience.ts` decides what is audible.
//
// Browsers only allow audio after a click or key press, so the audio graph is built on the first
// one. Sound pauses while the tab is hidden, and the on/off choice is remembered.

import { type AmbientMix, ambientMix, buildingCentres, type Listener } from './ambience';

/** Seconds between mix updates; the volume ramps hide the steps. */
const MIX_EVERY = 0.25;
/** Seconds for a layer's volume to cover about two thirds of a change. */
const RAMP = 0.6;
/** Overall volume and each layer's volume at full mix. */
const MASTER = 0.35;
const LAYER = { traffic: 0.45, birds: 0.2 };
/** Average seconds between bird calls at full birdsong. */
const BIRD_GAP = 1.2;
const STORAGE_KEY = 'sound';

interface Button {
  textContent: string | null;
  setAttribute(key: string, value: string): void;
  onclick: ((event: never) => unknown) | null;
}

interface Graph {
  ctx: AudioContext;
  master: GainNode;
  layers: Record<keyof AmbientMix, GainNode>;
}

export class AmbientSound {
  private on: boolean;
  private graph: Graph | null = null;
  private buildings: Float32Array = new Float32Array();
  private mixedAt = Number.NEGATIVE_INFINITY;
  private mix: AmbientMix = { traffic: 0, birds: 0 };
  private nextPass = 0;
  /** Audio-clock time of the next bird call. */
  private nextCall = 0;

  constructor(private readonly button: Button) {
    this.on = localStorage.getItem(STORAGE_KEY) !== 'off';
    button.onclick = () => this.toggle();
    this.drawButton();
    const start = () => this.start();
    addEventListener('pointerdown', start);
    addEventListener('keydown', start);
    document.addEventListener('visibilitychange', () => this.applyPower());
  }

  /** Turns sound on or off, and remembers the choice. */
  toggle() {
    this.on = !this.on;
    localStorage.setItem(STORAGE_KEY, this.on ? 'on' : 'off');
    this.drawButton();
    this.start();
    this.applyPower();
  }

  /** The buildings snapshot (see `buildings` in protocol.ts), whenever it changes. */
  setBuildings(data: Float32Array) {
    this.buildings = buildingCentres(data);
  }

  /** Called every frame with the agent snapshot, the sim clock and where the camera looks. */
  update(seconds: number, agents: Float32Array | undefined, secondOfDay: number, at: Listener) {
    const g = this.graph;
    if (g?.ctx.state !== 'running') return;
    if (seconds - this.mixedAt >= MIX_EVERY) {
      this.mixedAt = seconds;
      this.mix = ambientMix(agents, this.buildings, at, secondOfDay);
      for (const layer of ['traffic', 'birds'] as const)
        g.layers[layer].gain.setTargetAtTime(
          this.mix[layer] * LAYER[layer],
          g.ctx.currentTime,
          this.mix[layer] === 0 ? 0.1 : RAMP,
        );
    }
    this.scheduleTraffic(g);
    this.scheduleBirds(g);
  }

  private drawButton() {
    this.button.textContent = `sound ${this.on ? 'on' : 'off'} (N)`;
    this.button.setAttribute('aria-pressed', String(this.on));
  }

  /** Builds the audio graph on the first user gesture, if sound is on. */
  private start() {
    if (this.graph || !this.on || typeof AudioContext === 'undefined') return;
    this.graph = buildGraph(new AudioContext());
    this.applyPower();
  }

  /** Plays while on and visible, otherwise suspends the audio clock to save power. */
  private applyPower() {
    const g = this.graph;
    if (!g) return;
    const play = this.on && document.visibilityState !== 'hidden';
    g.master.gain.setTargetAtTime(play ? MASTER : 0, g.ctx.currentTime, 0.1);
    if (play) void g.ctx.resume();
    else void g.ctx.suspend();
  }

  /** Sparse traffic has separate, quiet passes, not an uninterrupted noise bed. */
  private scheduleTraffic(g: Graph) {
    const now = g.ctx.currentTime;
    if (this.mix.traffic < 0.002) {
      this.nextPass = now;
      return;
    }
    this.nextPass = Math.max(this.nextPass, now);
    if (this.nextPass < now + 0.3) {
      trafficPass(g.ctx, g.layers.traffic, this.nextPass);
      this.nextPass += (1.5 + Math.random() * 1.5) / Math.sqrt(this.mix.traffic);
    }
  }

  /** Schedules bird calls a little ahead, more often the more birdsong the mix asks for. */
  private scheduleBirds(g: Graph) {
    const now = g.ctx.currentTime;
    if (this.mix.birds < 0.02) {
      this.nextCall = now;
      return;
    }
    this.nextCall = Math.max(this.nextCall, now);
    while (this.nextCall < now + 0.3) {
      birdCall(g.ctx, g.layers.birds, this.nextCall);
      this.nextCall += (-Math.log(1 - Math.random()) * BIRD_GAP) / this.mix.birds;
    }
  }
}

function buildGraph(ctx: AudioContext): Graph {
  const master = ctx.createGain();
  master.gain.value = 0;
  master.connect(ctx.destination);
  const layer = () => {
    const gain = ctx.createGain();
    gain.gain.value = 0;
    gain.connect(master);
    return gain;
  };
  return { ctx, master, layers: { traffic: layer(), birds: layer() } };
}

/** A soft engine fundamental and overtone, with a short pass-by envelope instead of hiss. */
function trafficPass(ctx: AudioContext, out: AudioNode, at: number) {
  const duration = 2.4 + Math.random() * 0.6;
  const base = 58 + Math.random() * 34;
  const direction = Math.random() < 0.5 ? -1 : 1;
  const pan = new StereoPannerNode(ctx, { pan: direction * 0.45 });
  pan.pan.setValueAtTime(direction * 0.45, at);
  pan.pan.linearRampToValueAtTime(-direction * 0.45, at + duration);
  const filter = new BiquadFilterNode(ctx, { type: 'lowpass', frequency: 280, Q: 0.5 });
  const env = new GainNode(ctx, { gain: 0 });
  env.gain.setValueAtTime(0, at);
  env.gain.linearRampToValueAtTime(0.12, at + 0.4);
  env.gain.linearRampToValueAtTime(0.07, at + duration * 0.55);
  env.gain.linearRampToValueAtTime(0, at + duration);
  env.connect(filter).connect(pan).connect(out);
  const engine = new OscillatorNode(ctx, { type: 'triangle', frequency: base });
  const overtone = new OscillatorNode(ctx, { type: 'sine', frequency: base * 2 });
  const blend = new GainNode(ctx, { gain: 0.16 });
  engine.connect(env);
  overtone.connect(blend).connect(env);
  for (const [osc, pitch] of [
    [engine, base],
    [overtone, base * 2],
  ] as const) {
    osc.frequency.setValueAtTime(pitch * 1.06, at);
    osc.frequency.linearRampToValueAtTime(pitch * 0.88, at + duration);
    osc.start(at);
    osc.stop(at + duration + 0.02);
  }
  overtone.onended = () => {
    for (const node of [engine, overtone, blend, env, filter, pan]) node.disconnect();
  };
}

/** A few quick rising or falling chirps at `at`, panned somewhere left or right. */
function birdCall(ctx: AudioContext, out: AudioNode, at: number) {
  const pan = new StereoPannerNode(ctx, { pan: Math.random() * 1.6 - 0.8 });
  pan.connect(out);
  const base = 2500 + Math.random() * 2000;
  const sweep = (Math.random() < 0.5 ? 1 : -1) * (400 + Math.random() * 900);
  const chirps = 2 + Math.floor(Math.random() * 4);
  for (let i = 0; i < chirps; i++) {
    const t = at + i * 0.11;
    const osc = new OscillatorNode(ctx, { frequency: base });
    osc.frequency.setValueAtTime(base, t);
    osc.frequency.linearRampToValueAtTime(base + sweep, t + 0.08);
    const env = new GainNode(ctx, { gain: 0 });
    env.gain.setValueAtTime(0, t);
    env.gain.linearRampToValueAtTime(0.6, t + 0.01);
    env.gain.exponentialRampToValueAtTime(0.001, t + 0.09);
    osc.connect(env).connect(pan);
    osc.start(t);
    osc.stop(t + 0.1);
  }
}
