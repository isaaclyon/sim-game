import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';
import { initSync, Sim } from '../src/pkg/sim_wasm.js';

initSync({ module: readFileSync(new URL('../src/pkg/sim_wasm_bg.wasm', import.meta.url)) });

/** Runs sim.worker.ts against the real WASM sim with a fake clock. */
export async function worker(agents = 100, size = 3) {
  let ready;
  const initialized = new Promise((resolve) => {
    ready = resolve;
  });
  const messages = [];
  const self = { postMessage: (msg) => messages.push(msg) };
  // Fake time: `advance(ms)` moves the clock and runs the worker's next step.
  let now = 0;
  let nextStep = () => {};
  const advance = (ms) => {
    now += ms;
    nextStep();
  };
  const context = createContext({
    self,
    performance: { now: () => now },
    setTimeout: (fn) => {
      nextStep = fn;
    },
  });
  const source = stripTypeScriptTypes(
    readFileSync(new URL('../src/sim.worker.ts', import.meta.url), 'utf8'),
  );
  const module = new SourceTextModule(source, { context });
  const wasm = new SyntheticModule(
    ['default', 'Sim'],
    function () {
      this.setExport('default', () => initialized);
      this.setExport('Sim', Sim);
    },
    { context },
  );
  await module.link(() => wasm);
  await module.evaluate();
  const send = (data) => self.onmessage({ data });
  const init = send({ type: 'init', seed: 1, cols: size, rows: size, agents });
  return { send, messages, ready, init, advance };
}
