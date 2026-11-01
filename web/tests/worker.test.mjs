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

test('curve preview, build and bulldoze agree across the worker boundary while paused', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const before = w.messages.find((msg) => msg.type === 'roads').segments;
  const coords = { ax: -1000, ay: -1000, cx: -900, cy: -900, bx: -800, by: -1000 };
  await w.send({ type: 'previewCurve', ...coords, revision: 10 });
  const preview = w.messages.at(-1);
  assert.equal(preview.revision, 10);
  assert.ok(preview.stops.length > 6);
  await w.send({ type: 'buildCurve', ...coords, class: 0, id: 20 });
  const result = w.messages.at(-1);
  assert.equal(result.type, 'buildResult');
  assert.equal(result.id, 20);
  assert.equal(result.success, true);
  const added = w.messages.at(-2).segments.slice(before.length);
  assert.equal(added.length / 5, preview.stops.length / 3 - 1);
  await w.send({ type: 'previewBulldoze', x: -900, y: -950, revision: 11 });
  const selected = w.messages.at(-1);
  assert.equal(selected.revision, 11);
  assert.equal(selected.segment.length / 2, added.length / 5 + 1);
  for (let i = 0; i < added.length / 5; i++) {
    assert.deepEqual(
      [...selected.segment.slice(i * 2, i * 2 + 4)],
      [...added.slice(i * 5, i * 5 + 4)],
    );
  }
  await w.send({ type: 'deleteRoad', x: -900, y: -950 });
  assert.deepEqual(w.messages.at(-2).segments, before);
  assert.equal(w.messages.at(-1).type, 'frame');
  assert.equal(w.messages.at(-1).stats.tick, 0);
  await w.send({ type: 'buildCurve', ...coords, class: 0, id: 21 });
  assert.equal(w.messages.at(-1).success, true, 'a deleted curve does not block rebuilding');
  assert.deepEqual(w.messages.at(-2).segments.slice(before.length), added);
  const count = w.messages.length;
  await w.send({ type: 'buildCurve', ...coords, class: 0, id: 22 });
  assert.equal(w.messages.length, count + 1, 'refused builds do not publish a changed network');
  assert.equal(w.messages.at(-1).id, 22);
  assert.equal(w.messages.at(-1).success, false);
});

test('every preview request gets a reply even during WASM initialization', async () => {
  const w = await worker();
  for (const type of ['previewBulldoze', 'previewSnap', 'previewRoad', 'previewCurve']) {
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
  const painted = w.messages.findLast((msg) => msg.type === 'zones');
  assert.notEqual(painted, first);
  assert.equal(w.messages.at(-1).type, 'frame', 'zoning can add alleys, which can move cars');
  assert.deepEqual([...painted.cells.slice(0, 4)], [2, 2, 2, 0]);
  assert.deepEqual([...painted.cells.slice(cols, cols + 4)], [2, 2, 2, 0]);
  assert.equal(painted.cells[2 * cols], 0);
  const count = w.messages.length;
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 2 });
  assert.equal(w.messages.length, count, 'a paint that changes nothing sends nothing');
  await w.send({ type: 'paintZone', ax: x0, ay: y0, bx: x0 + 24, by: y0 + 16, zone: 9 });
  assert.equal(w.messages.length, count, 'unknown zones are refused');
});

test('a click fills the block under it, and its lots are sent', async () => {
  const w = await worker();
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const lots = w.messages.find((msg) => msg.type === 'lots');
  assert.equal(lots.rings.length, 0, 'no lots before zoning');
  // The generated town's first block lies between its first two parallel roads.
  const roads = w.messages.find((msg) => msg.type === 'roads').segments;
  const [x0, y0] = [Math.min(roads[0], roads[2]), Math.min(roads[1], roads[3])];
  const [x, y] = [x0 + 50, y0 + 50];
  await w.send({ type: 'previewBlock', x, y, revision: 3 });
  const outline = w.messages.at(-1);
  assert.equal(outline.type, 'blockPreview');
  assert.equal(outline.revision, 3);
  assert.ok(outline.outline.length >= 8, 'a closed block has at least four corners');
  await w.send({ type: 'previewBlock', x: x0 - 500, y: y0 - 500, revision: 4 });
  assert.equal(w.messages.at(-1).outline.length, 0, 'no block outside the town');
  const count = w.messages.length;
  await w.send({ type: 'fillBlock', x: x0 - 500, y: y0 - 500, zone: 1 });
  assert.equal(w.messages.length, count, 'filling outside every block changes nothing');
  await w.send({ type: 'fillBlock', x, y, zone: 1 });
  const zones = w.messages.findLast((msg) => msg.type === 'zones');
  assert.ok(zones.cells.some((c) => c === 1));
  const cut = w.messages.findLast((msg) => msg.type === 'lots');
  assert.ok(cut.rings.length > 0);
  // Every ring is a point count of at least three, then that many points, end to end.
  let i = 0;
  while (i < cut.rings.length) {
    assert.ok(cut.rings[i] >= 3);
    i += 1 + 2 * cut.rings[i];
  }
  assert.equal(i, cut.rings.length);
  assert.equal(w.messages.at(-1).type, 'frame');
  assert.equal(w.messages.at(-1).stats.tick, 0, 'filling does not advance time');
});
