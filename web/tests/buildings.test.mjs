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
    assert.ok(data[i + 2] >= 3);
    i += 3 + 2 * data[i + 2];
  }
  assert.equal(i, data.length);
  const version = s.buildings_version();
  assert.ok(s.fill_block(50, 50, 0));
  s.tick(610);
  assert.deepEqual(s.building_snapshot(), data);
  assert.equal(s.buildings_version(), version);
  s.free();
});

test('worker publishes growing buildings and paused road edits, but no unchanged geometry', async () => {
  let now = 0;
  let step;
  const messages = [];
  const self = { postMessage: (msg) => messages.push(msg) };
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
  assert.equal(buildings().length, 1);
  assert.equal(buildings()[0].data.length, 0);
  await send({ type: 'fillBlock', x: 50, y: 50, zone: 2 });
  for (let i = 0; i < 61; i++) {
    now += 1000;
    step();
  }
  const grown = buildings().at(-1);
  assert.ok(grown.data.length > 0);
  const count = buildings().length;
  now += 1000;
  step();
  assert.equal(buildings().length, count);
  await send({ type: 'setSpeed', speed: 0 });
  await send({ type: 'buildRoad', ax: 0, ay: 60, bx: 120, by: 60, class: 0, id: 1 });
  assert.notDeepEqual(
    buildings().at(-1).data,
    grown.data,
    'paused construction removes displaced buildings immediately',
  );
});
