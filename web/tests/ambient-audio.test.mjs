import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createContext, SourceTextModule } from 'node:vm';

/** Records Web Audio scheduling and connections; it does not assess how the synthesis sounds. */
async function audio() {
  const contexts = [];
  const events = new Map();
  const stored = new Map();
  const param = (value = 0) => ({
    value,
    calls: [],
    setTargetAtTime(...args) {
      this.calls.push(args);
    },
    setValueAtTime(...args) {
      this.calls.push(args);
    },
    linearRampToValueAtTime(...args) {
      this.calls.push(args);
    },
    exponentialRampToValueAtTime(...args) {
      this.calls.push(args);
    },
  });
  class Node {
    constructor(ctx, kind, options = {}) {
      this.kind = kind;
      this.gain = param(options.gain ?? 1);
      this.frequency = param(options.frequency ?? 440);
      this.pan = param(options.pan ?? 0);
      this.connections = [];
      Object.assign(this, options);
      if (typeof this.frequency === 'number') this.frequency = param(this.frequency);
      if (typeof this.gain === 'number') this.gain = param(this.gain);
      if (typeof this.pan === 'number') this.pan = param(this.pan);
      ctx.nodes.push(this);
    }
    connect(node) {
      this.connections.push(node);
      return node;
    }
    disconnect() {
      this.disconnected = true;
    }
    start(...args) {
      this.started = args;
    }
    stop(at) {
      this.stopped = at;
    }
  }
  class AudioContext {
    state = 'running';
    currentTime = 0;
    sampleRate = 100;
    nodes = [];
    destination = {};
    constructor() {
      contexts.push(this);
    }
    createGain() {
      return new Node(this, 'gain');
    }
    createBufferSource() {
      return new Node(this, 'buffer');
    }
    createBuffer(_channels, size) {
      return { getChannelData: () => new Float32Array(size) };
    }
    resume() {
      this.state = 'running';
      return Promise.resolve();
    }
    suspend() {
      this.state = 'suspended';
      return Promise.resolve();
    }
  }
  const nodeClass = (kind) =>
    class extends Node {
      constructor(ctx, options) {
        super(ctx, kind, options);
      }
    };
  const math = Object.create(Math);
  math.random = () => 0.5;
  const document = {
    visibilityState: 'visible',
    addEventListener: (name, fn) => events.set(name, fn),
  };
  const context = createContext({
    Math: math,
    AudioContext,
    document,
    GainNode: nodeClass('gain'),
    OscillatorNode: nodeClass('oscillator'),
    BiquadFilterNode: nodeClass('filter'),
    StereoPannerNode: nodeClass('pan'),
    localStorage: {
      getItem: (key) => stored.get(key),
      setItem: (key, value) => stored.set(key, value),
    },
    addEventListener: (name, fn) => events.set(name, fn),
  });
  const modules = new Map();
  async function source(name) {
    if (modules.has(name)) return modules.get(name);
    const module = new SourceTextModule(
      stripTypeScriptTypes(readFileSync(new URL(`../src/${name}.ts`, import.meta.url), 'utf8'), {
        mode: 'transform',
      }),
      { context },
    );
    modules.set(name, module);
    await module.link((specifier) => source(specifier.replace(/^\.\//, '').replace(/\.ts$/, '')));
    return module;
  }
  const module = await source('ambient-sound');
  await module.evaluate();
  const button = { setAttribute() {}, textContent: '', onclick: null };
  const sound = new module.namespace.AmbientSound(button);
  events.get('pointerdown')();
  const ctx = contexts[0];
  return {
    sound,
    ctx,
    document,
    events,
    button,
    update: (seconds, cars, distance = 50, secondOfDay = 0) => {
      ctx.currentTime = seconds;
      sound.update(seconds, cars, secondOfDay, { x: 0, y: 0, distance });
    },
  };
}
const cars = (n, ratio = 1) =>
  new Float32Array(Array.from({ length: n }, (_, i) => [i, 0, 0, ratio, 0]).flat());
const oscillators = (ctx) => ctx.nodes.filter((node) => node.kind === 'oscillator');

test('a town with buildings but no drivers has no ongoing traffic or static loop', async () => {
  const t = await audio();
  const buildings = Array.from({ length: 15 }, (_, i) => [
    1,
    6,
    0,
    0,
    4,
    i,
    0,
    i + 10,
    0,
    i + 10,
    10,
    i,
    10,
  ]);
  t.sound.setBuildings(new Float32Array(buildings.flat()));
  for (let second = 0; second < 10; second++) t.update(second, cars(0));
  for (let second = 10; second < 20; second++) t.update(second, cars(0), 3000, 12 * 3600);
  assert.equal(
    t.ctx.nodes.filter((node) => node.kind === 'buffer').length,
    0,
    'no continuous noise sources',
  );
  assert.equal(oscillators(t.ctx).length, 0, 'no engine voices without drivers');
});

test('moving cars get brief engine voices that stop and release their audio nodes', async () => {
  const t = await audio();
  t.update(0, cars(20));
  const voices = oscillators(t.ctx);
  assert.ok(voices.length > 0, 'nearby moving traffic schedules a passing engine');
  for (const voice of voices) {
    assert.ok(voice.frequency.value < 300, 'low engine tones, not a bright hiss');
    assert.ok(voice.stopped > 0 && voice.stopped <= 4, 'every voice has a short scheduled end');
  }
  for (const voice of voices) voice.onended?.();
  const transient = t.ctx.nodes.filter(
    (node) => node.started || node.kind === 'pan' || node.kind === 'filter',
  );
  assert.ok(
    transient.every((node) => node.disconnected),
    'finished voices release their connections',
  );
  const count = voices.length;
  for (let second = 1; second < 10; second++) t.update(second, cars(20, 0));
  assert.equal(oscillators(t.ctx).length, count, 'stopped traffic schedules no more pass-bys');
  assert.equal(t.ctx.nodes.filter((node) => node.kind === 'buffer').length, 0);
});

test('muting and hiding the tab suspend sound without queuing traffic bursts', async () => {
  const t = await audio();
  t.sound.toggle();
  t.update(1, cars(30));
  assert.equal(t.ctx.state, 'suspended');
  assert.equal(oscillators(t.ctx).length, 0);
  t.sound.toggle();
  t.update(20, cars(30));
  assert.ok(
    oscillators(t.ctx).length <= 2,
    'resuming schedules only one engine, not missed events',
  );
  const count = oscillators(t.ctx).length;
  t.document.visibilityState = 'hidden';
  t.events.get('visibilitychange')();
  t.update(21, cars(30));
  assert.equal(t.ctx.state, 'suspended');
  assert.equal(oscillators(t.ctx).length, count);
});
