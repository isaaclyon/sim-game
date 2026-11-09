import assert from 'node:assert/strict';
import test from 'node:test';
import { Sim } from '../src/pkg/sim_wasm.js';
import { coalSpot, worker as makeWorker, pump } from './worker-harness.mjs';

test('WASM sends sim-owned building footprints, fronts, heights and original zones', () => {
  const s = new Sim(1, 3, 3);
  assert.equal(s.building_snapshot().length, 0);
  pump(s);
  assert.ok(s.fill_block(50, 50, 2));
  s.tick(610);
  const data = s.building_snapshot();
  assert.ok(data.length > 0);
  let i = 0;
  while (i < data.length) {
    assert.equal(data[i], 2);
    assert.ok(data[i + 1] >= 12);
    assert.equal(data[i + 2], 0, 'non-industrial roofs keep their original palette');
    assert.ok(data[i + 3] < data[i + 5], 'the front is one of the footprint edges');
    assert.equal(data[i + 4], 1, 'a generated grid builds in the town era');
    assert.ok(data[i + 5] >= 3);
    i += 6 + 2 * data[i + 5];
  }
  assert.equal(i, data.length);
  const version = s.buildings_version();
  assert.ok(s.fill_block(50, 50, 0));
  s.tick(20);
  assert.deepEqual(s.building_snapshot(), data, 'unzoned buildings close gradually');
  assert.equal(s.buildings_version(), version);
  s.tick(20000);
  assert.equal(s.building_snapshot().length, 0, 'closed buildings leave unzoned land vacant');
  assert.notEqual(s.buildings_version(), version);
  s.free();
});

test('industrial snapshots keep varied roofs and halls until repainting closes them', () => {
  const s = new Sim(1, 6, 6);
  const coal = coalSpot(s);
  assert.ok(s.place_plant(0, coal.x, coal.y), 'more than starter power');
  pump(s);
  // A populated block supplies enough demand for variety; an empty town now needs only
  // one high-capacity factory, which is the intended industrial land balance.
  assert.ok(s.fill_block(60, 300, 2));
  assert.ok(s.fill_block(300, 300, 4));
  s.tick(1800);
  assert.ok(s.fill_block(50, 50, 6));
  s.tick(2400);
  const data = s.building_snapshot();
  const roofs = new Set();
  const heights = new Set();
  let i = 0;
  while (i < data.length) {
    if (data[i] === 2 || data[i] === 4) {
      i += 6 + 2 * data[i + 5];
      continue;
    }
    assert.equal(data[i], 6);
    assert.ok(data[i + 2] >= 0 && data[i + 2] <= 3);
    roofs.add(data[i + 2]);
    heights.add(data[i + 1]);
    assert.equal(data[i + 5], 4, 'industrial halls stay rectangular');
    i += 6 + 2 * data[i + 5];
  }
  assert.equal(i, data.length);
  assert.ok(roofs.size > 1);
  assert.ok(heights.size > 1);
  s.fill_block(50, 50, 1);
  assert.deepEqual(s.building_snapshot(), data, 'repainting keeps halls until they close');
  // Workplaces close at their first step, 3–12 sim seconds. Removing their jobs leaves
  // housing oversupplied in this populated fixture, so replacement houses wait for demand.
  s.tick(730);
  const zones = new Set();
  const after = s.building_snapshot();
  for (let j = 0; j < after.length; ) {
    zones.add(after[j]);
    j += 6 + 2 * after[j + 5];
  }
  assert.deepEqual(
    zones,
    new Set([2, 4]),
    'closed industry disappears while the original housing and shops remain',
  );
  s.free();
});

async function worker() {
  const w = await makeWorker(0, 3);
  w.ready();
  await w.init;
  await w.water();
  return { ...w, buildings: () => w.messages.filter((m) => m.type === 'buildings') };
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
  await send({ type: 'buildRoad', points: [0, 60, 0, 120, 60, 0], class: 0, id: 1 });
  assert.notDeepEqual(
    buildings().at(-1).data,
    grown.data,
    'paused construction removes displaced buildings immediately',
  );
});

test('worker publishes buildings removed by closure while running', async () => {
  const { send, buildings, advance } = await worker();
  await send({ type: 'fillBlock', x: 50, y: 50, zone: 6 });
  for (let i = 0; i < 61; i++) advance(1000);
  assert.ok(buildings().at(-1).data.length > 0);
  await send({ type: 'fillBlock', x: 50, y: 50, zone: 0 });
  assert.ok(buildings().at(-1).data.length > 0, 'repainting keeps buildings at first');
  for (let i = 0; i < 13; i++) advance(1000);
  assert.equal(buildings().at(-1).data.length, 0);
});

test('fast growth limits full geometry updates and pausing flushes the latest buildings', async () => {
  const w = await worker();
  await w.power();
  await w.water();
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
  const coal = coalSpot(mirror);
  mirror.place_plant(0, coal.x, coal.y);
  pump(mirror);
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
