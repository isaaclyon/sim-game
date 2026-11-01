import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import test from 'node:test';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';
import { initSync, Sim } from '../src/pkg/sim_wasm.js';

initSync({ module: readFileSync(new URL('../src/pkg/sim_wasm_bg.wasm', import.meta.url)) });

test('WASM sends sim-owned building footprints, heights and original zones', () => {
  const s = new Sim(1, 3, 3);
  assert.equal(s.building_snapshot().length, 0);
  assert.ok(s.fill_block(50, 50, 2));
  s.tick(610);
  const data = s.building_snapshot();
  assert.ok(data.length > 0);
  let i = 0;
  while (i < data.length) {
    assert.equal(data[i], 2);
    assert.ok(data[i + 1] >= 12);
    assert.equal(data[i + 2], 0, 'non-industrial roofs keep their original palette');
    assert.ok(data[i + 3] >= 3);
    i += 4 + 2 * data[i + 3];
  }
  assert.equal(i, data.length);
  const version = s.buildings_version();
  assert.ok(s.fill_block(50, 50, 0));
  s.tick(610);
  assert.deepEqual(s.building_snapshot(), data);
  assert.equal(s.buildings_version(), version);
  s.free();
});

test('industrial snapshots carry varied roofs and halls and preserve them after repainting', () => {
  const s = new Sim(1, 3, 3);
  assert.ok(s.fill_block(50, 50, 6));
  s.tick(610);
  const data = s.building_snapshot();
  const roofs = new Set();
  const heights = new Set();
  let i = 0;
  while (i < data.length) {
    assert.equal(data[i], 6);
    assert.ok(data[i + 2] >= 0 && data[i + 2] <= 3);
    roofs.add(data[i + 2]);
    heights.add(data[i + 1]);
    assert.equal(data[i + 3], 4, 'industrial halls stay rectangular');
    i += 4 + 2 * data[i + 3];
  }
  assert.equal(i, data.length);
  assert.ok(roofs.size > 1);
  assert.ok(heights.size > 1);
  s.fill_block(50, 50, 1);
  assert.deepEqual(s.building_snapshot(), data);
  s.tick(610);
  // Vacant corner lots may now fit houses; existing halls must remain unchanged.
  assert.deepEqual(s.building_snapshot().subarray(0, data.length), data);
  s.free();
});

async function worker() {
  let now = 0;
  let step;
  const messages = [];
  const publishedAt = [];
  const self = {
    postMessage: (msg) => {
      messages.push(msg);
      if (msg.type === 'buildings') publishedAt.push(now);
    },
  };
  const context = createContext({
    self,
    performance: { now: () => now },
    setTimeout: (fn) => {
      step = fn;
    },
  });
  const module = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/sim.worker.ts', import.meta.url), 'utf8')),
    { context },
  );
  const wasm = new SyntheticModule(
    ['default', 'Sim'],
    function () {
      this.setExport('default', async () => {});
      this.setExport('Sim', Sim);
    },
    { context },
  );
  await module.link(() => wasm);
  await module.evaluate();
  const send = (data) => self.onmessage({ data });
  await send({ type: 'init', seed: 1, cols: 3, rows: 3, agents: 0 });
  const buildings = () => messages.filter((m) => m.type === 'buildings');
  return {
    send,
    messages,
    buildings,
    publishedAt,
    advance(ms) {
      now += ms;
      step();
    },
  };
}

test('worker publishes growing buildings and paused road edits, but no unchanged geometry', async () => {
  const { send, buildings, advance } = await worker();
  assert.equal(buildings().length, 1);
  assert.equal(buildings()[0].data.length, 0);
  await send({ type: 'fillBlock', x: 50, y: 50, zone: 2 });
  for (let i = 0; i < 61; i++) {
    advance(1000);
  }
  const grown = buildings().at(-1);
  assert.ok(grown.data.length > 0);
  const count = buildings().length;
  advance(1000);
  assert.equal(buildings().length, count);
  await send({ type: 'setSpeed', speed: 0 });
  await send({ type: 'buildRoad', ax: 0, ay: 60, bx: 120, by: 60, class: 0, id: 1 });
  assert.notDeepEqual(
    buildings().at(-1).data,
    grown.data,
    'paused construction removes displaced buildings immediately',
  );
});

test('fast growth limits full geometry updates and pausing flushes the latest buildings', async () => {
  const w = await worker();
  await w.send({ type: 'fillBlock', x: 50, y: 50, zone: 2 });
  await w.send({ type: 'setSpeed', speed: 10 });
  for (let i = 0; i < 100; i++) w.advance(10);
  for (let i = 1; i < w.publishedAt.length; i++) {
    assert.ok(
      w.publishedAt[i] - w.publishedAt[i - 1] >= 250,
      'whole-city mesh rebuilds are capped at four per real second',
    );
  }
  const mirror = new Sim(1, 3, 3);
  mirror.fill_block(50, 50, 2);
  mirror.tick(w.messages.findLast((m) => m.type === 'frame').stats.tick);
  const expected = mirror.building_snapshot();
  assert.ok(expected.length > 0);
  await w.send({ type: 'setSpeed', speed: 0 });
  assert.deepEqual(
    w.buildings().at(-1).data,
    expected,
    'pause must not leave a throttled snapshot hidden indefinitely',
  );
  mirror.free();
});
