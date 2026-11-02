// Ambient sound that follows the camera: traffic hum, city noise and birds, synthesised with Web
// Audio so there are no sample files to license or download. `ambience.ts` decides how loud each
// layer is; this only plays it.
//
// Browsers only allow audio after a click or key press, so the audio graph is built on the first
// one. Sound pauses while the tab is hidden, and the on/off choice is remembered.

import { type AmbientMix, ambientMix, buildingCentres, type Listener } from './ambience';

/** Seconds between mix updates; the volume ramps hide the steps. */
const MIX_EVERY = 0.25;
/** Seconds for a layer's volume to cover about two thirds of a change. */
const RAMP = 0.6;
/** Overall volume and each layer's volume at full mix. */
const MASTER = 0.5;
const LAYER = { traffic: 0.9, city: 0.35, birds: 0.25 };
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
  private mix: AmbientMix = { traffic: 0, city: 0, birds: 0 };
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
      for (const layer of ['traffic', 'city', 'birds'] as const)
        g.layers[layer].gain.setTargetAtTime(
          this.mix[layer] * LAYER[layer],
          g.ctx.currentTime,
          RAMP,
        );
    }
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
  const layers = { traffic: layer(), city: layer(), birds: layer() };
  const noise = ctx.createBuffer(1, ctx.sampleRate * 4, ctx.sampleRate);
  const samples = noise.getChannelData(0);
  for (let i = 0; i < samples.length; i++) samples[i] = Math.random() * 2 - 1;
  const play = (offset: number, ...chain: AudioNode[]) => {
    const source = ctx.createBufferSource();
    source.buffer = noise;
    source.loop = true;
    let node: AudioNode = source;
    for (const next of chain) node = node.connect(next);
    source.start(0, offset);
  };
  const filter = (type: BiquadFilterType, frequency: number, Q = 0.7) =>
    new BiquadFilterNode(ctx, { type, frequency, Q });
  // Traffic: a deep rumble that slowly swells and fades, like cars passing.
  const swell = ctx.createGain();
  const lfo = new OscillatorNode(ctx, { frequency: 0.13 });
  const depth = new GainNode(ctx, { gain: 0.3 });
  lfo.connect(depth).connect(swell.gain);
  lfo.start();
  play(0, filter('lowpass', 160), filter('lowpass', 420), swell, layers.traffic);
  // City: a band of mid-range noise, like distant voices, air vents and footsteps.
  play(1.7, filter('bandpass', 900, 0.5), filter('lowpass', 2500), layers.city);
  return { ctx, master, layers };
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
