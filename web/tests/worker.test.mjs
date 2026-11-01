import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';
import { initSync, Sim } from '../src/pkg/sim_wasm.js';

initSync({ module: readFileSync(new URL('../src/pkg/sim_wasm_bg.wasm', import.meta.url)) });

async function worker() {
  let ready;
  const initialized = new Promise((resolve) => {
    ready = resolve;
  });
  const messages = [];
  const self = { postMessage: (msg) => messages.push(msg) };
  const context = createContext({ self, performance: { now: () => 0 }, setTimeout: () => {} });
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
  const init = send({ type: 'init', seed: 1, cols: 3, rows: 3, agents: 100 });
  return { send, messages, ready, init };
}

test('every preview request gets a reply even during WASM initialization', async () => {
  const w = await worker();
  for (const type of ['previewBulldoze', 'previewSnap', 'previewRoad']) {
    await w.send({ type, x: 0, y: 0, ax: 0, ay: 0, bx: 100, by: 0, revision: 7 });
    assert.equal(w.messages.at(-1)?.revision, 7);
    const reply = w.messages.pop();
    assert.equal((reply.segment ?? reply.stops).length, 0);
  }
  w.ready();
  await w.init;
  const roads = w.messages.find((msg) => msg.type === 'roads').segments;
  const x = (roads[0] + roads[2]) / 2;
  const y = (roads[1] + roads[3]) / 2;
  await w.send({ type: 'previewBulldoze', x, y, revision: 8 });
  assert.equal(w.messages.at(-1).segment.length, 4);
  await w.send({ type: 'setSpeed', speed: 0 });
  await w.send({ type: 'deleteRoad', x, y });
  assert.equal(w.messages.at(-2).type, 'roads');
  assert.equal(w.messages.at(-2).segments.length, roads.length - 5);
  assert.equal(w.messages.at(-1).type, 'frame', 'paused edits refresh cars and signals');
  assert.equal(w.messages.at(-1).stats.tick, 0, 'deletion does not advance time');
});

test('the worker sends the zone grid on start and after each paint that changes it', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  const first = w.messages.find((msg) => msg.type === 'zones');
  const [x0, y0, cell, cols, rows] = first.layout;
  assert.equal(cell, 8);
  assert.equal(first.cells.length, cols * rows);
  assert.ok(first.cells.every((c) => c === 0));
  // Residential high density over the first 3 x 2 cells.
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 2 });
  const painted = w.messages.at(-1);
  assert.equal(painted.type, 'zones');
  assert.deepEqual([...painted.cells.slice(0, 4)], [2, 2, 2, 0]);
  assert.deepEqual([...painted.cells.slice(cols, cols + 4)], [2, 2, 2, 0]);
  assert.equal(painted.cells[2 * cols], 0);
  const count = w.messages.length;
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 2 });
  assert.equal(w.messages.length, count, 'a paint that changes nothing sends nothing');
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 9 });
  assert.equal(w.messages.length, count, 'unknown zones are refused');
});
