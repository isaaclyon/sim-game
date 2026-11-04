import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';
import { initSync, Sim } from '../src/pkg/sim_wasm.js';

initSync({ module: readFileSync(new URL('../src/pkg/sim_wasm_bg.wasm', import.meta.url)) });

/** Where a coal plant fits west of a fresh city, so growth fixtures are not held back by
 * starter power (ADR 0041). `sim` is a `Sim` with the city's seed and size. */
export function coalSpot(sim) {
  for (let y = 0; y < 400; y += 10)
    for (let x = -20; x > -60; x -= 2) if (sim.plant_preview(0, x, y)[0] === 1) return { x, y };
  throw new Error('no room for a coal plant');
}

/** Runs sim.worker.ts against the real WASM sim with a fake clock and local save slot. */
export async function worker(agents = 100, size = 3, storage, grid = true) {
  let ready;
  const initialized = new Promise((resolve) => {
    ready = resolve;
  });
  const messages = [];
  const publishedAt = [];
  const self = {
    postMessage: (msg) => {
      messages.push(msg);
      if (msg.type === 'buildings') publishedAt.push(now);
    },
  };
  let record = null;
  storage ??= {
    async read() {
      return record;
    },
    async write(value) {
      record = value;
    },
  };
  let now = 0;
  let nextStep = () => {};
  const intervals = [];
  const advance = (ms) => {
    now += ms;
    nextStep();
    for (const timer of intervals)
      if (now >= timer.at) {
        timer.at = now + timer.ms;
        timer.fn();
      }
  };
  const context = createContext({
    self,
    performance: { now: () => now },
    Date,
    setTimeout: (fn) => {
      nextStep = fn;
    },
    setInterval: (fn, ms) => {
      intervals.push({ fn, ms, at: now + ms });
    },
  });
  const wasm = new SyntheticModule(
    ['default', 'Sim'],
    function () {
      this.setExport('default', () => initialized);
      this.setExport('Sim', Sim);
    },
    { context },
  );
  const store = new SyntheticModule(
    ['cityStorage'],
    function () {
      this.setExport('cityStorage', storage);
    },
    { context },
  );
  const modules = new Map();
  async function source(name) {
    if (modules.has(name)) return modules.get(name);
    const module = new SourceTextModule(
      stripTypeScriptTypes(readFileSync(new URL(`../src/${name}.ts`, import.meta.url), 'utf8')),
      { context },
    );
    modules.set(name, module);
    await module.link((specifier) =>
      specifier.endsWith('/sim_wasm')
        ? wasm
        : specifier === './city-storage'
          ? store
          : source(specifier.slice(2)),
    );
    return module;
  }
  const module = await source('sim.worker');
  await module.evaluate();
  const send = (data) => self.onmessage({ data });
  const init = send({ type: 'init', seed: 1, cols: size, rows: size, agents, grid });
  return {
    send,
    messages,
    publishedAt,
    ready,
    init,
    advance,
    settled: () => new Promise(setImmediate),
    /** Places a coal plant beside the city; the worker must be initialised. */
    async power() {
      const mirror = new Sim(1, size, size);
      const spot = coalSpot(mirror);
      mirror.free();
      await send({ type: 'placePlant', kind: 0, ...spot });
    },
  };
}
