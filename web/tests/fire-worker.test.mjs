import assert from 'node:assert/strict';
import test from 'node:test';
import { Sim } from '../src/pkg/sim_wasm.js';
import { worker } from './worker-harness.mjs';

const last = (w, type) => w.messages.findLast((m) => m.type === type);

/** Where a station fits beside the generated grid's west edge. */
function stationSpot() {
  const sim = new Sim(1, 3, 3);
  try {
    for (let y = 30; y < 400; y += 10)
      for (let x = -10; x > -60; x -= 2) if (sim.station_preview(x, y)[0] === 1) return { x, y };
  } finally {
    sim.free();
  }
  throw new Error('no room for a fire station');
}

test('a fire station is previewed, placed, paid for and drawn with its coverage', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const first = last(w, 'fire');
  assert.equal(first.stations.length, 0, 'a fresh city has no stations');
  assert.equal(first.coverRoads.length, 0);
  const before = last(w, 'frame').stats;
  assert.equal(before.fire.enabled, true, 'fires are on by default');
  assert.equal(before.fire.stations, 0);
  assert.equal(before.fire.threshold, 1000);

  const spot = stationSpot();
  await w.send({ type: 'previewStation', ...spot, revision: 7 });
  const preview = last(w, 'stationPreview');
  assert.equal(preview.revision, 7);
  assert.equal(preview.ok, true);
  assert.ok(preview.outline.length >= 10, 'a closed square');
  await w.send({ type: 'previewStation', x: 5000, y: 5000, revision: 8 });
  assert.equal(last(w, 'stationPreview').outline.length, 0, 'no road in reach');

  await w.send({ type: 'placeStation', ...spot });
  const fire = last(w, 'fire');
  assert.equal(fire.stations.length, 5, '[x, y, half, frontX, frontY]');
  assert.ok(fire.coverRoads.length > 0, 'the station reaches roads');
  const after = last(w, 'frame').stats;
  assert.equal(after.fire.stations, 1);
  assert.equal(after.fire.trucks, 2);
  assert.equal(after.fire.freeTrucks, 2);
  assert.ok(after.balance < before.balance, 'the station is paid for');
  assert.ok(after.upkeep > before.upkeep, 'and costs upkeep');

  await w.send({ type: 'placeStation', ...spot });
  assert.equal(last(w, 'frame').stats.fire.stations, 1, 'a second station cannot overlap');
});

test('the fire setting is the sim’s, and turning it off is an edit', async () => {
  const w = await worker(0);
  w.ready();
  await w.init;
  await w.send({ type: 'setSpeed', speed: 0 });
  const frames = w.messages.filter((m) => m.type === 'frame').length;
  await w.send({ type: 'setFires', on: false });
  assert.equal(w.messages.filter((m) => m.type === 'frame').length, frames + 1);
  assert.equal(last(w, 'frame').stats.fire.enabled, false);
  await w.send({ type: 'setFires', on: false });
  assert.equal(
    w.messages.filter((m) => m.type === 'frame').length,
    frames + 1,
    'no change, no message',
  );
  await w.send({ type: 'setFires', on: true });
  assert.equal(last(w, 'frame').stats.fire.enabled, true);
});
