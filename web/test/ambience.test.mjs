import assert from 'node:assert/strict';
import test from 'node:test';
import { ambientMix, buildingCentres, daylight } from '../src/ambience.ts';

const NOON = 12 * 3600;
const MIDNIGHT = 0;
/** Agent snapshot: `[x, y, speedRatio, kind]` per agent, heading 0. */
const agents = (...list) => new Float32Array(list.flatMap(([x, y, r, k]) => [x, y, 0, r, k]));
/** `n` moving cars spread around (x, y). */
const traffic = (x, y, n) =>
  agents(
    ...Array.from({ length: n }, (_, i) => [x + (i % 5) * 4, y + Math.floor(i / 5) * 4, 1, 0]),
  );
/** A 10 m square building of `height` metres centred on (x, y), in the buildings snapshot format. */
const square = (x, y, height) => [
  1,
  height,
  0,
  2,
  1,
  4,
  x - 5,
  y - 5,
  x + 5,
  y - 5,
  x + 5,
  y + 5,
  x - 5,
  y + 5,
];
const block = (x, y, n, height) =>
  buildingCentres(
    new Float32Array(
      Array.from({ length: n }, (_, i) =>
        square(x + (i % 5) * 15, y + Math.floor(i / 5) * 15, height),
      ).flat(),
    ),
  );
const at = (x, y, distance = 100) => ({ x, y, distance });
const none = new Float32Array();

test('buildingCentres keeps each footprint centre and height', () => {
  const data = new Float32Array([...square(10, 20, 8), ...square(-30, 5, 40)]);
  assert.deepEqual([...buildingCentres(data)], [10, 20, 8, -30, 5, 40]);
});

test('traffic is loud over busy roads and fades away from them', () => {
  const cars = traffic(0, 0, 30);
  const over = ambientMix(cars, none, at(0, 0), NOON);
  const away = ambientMix(cars, none, at(2000, 0), NOON);
  assert.ok(over.traffic > 0.2, `busy road remains audible: ${over.traffic}`);
  assert.equal(away.traffic, 0, 'cars out of earshot are silent');
  const stopped = agents(...Array.from({ length: 30 }, (_, i) => [i, 0, 0, 0]));
  assert.ok(ambientMix(stopped, none, at(0, 0), NOON).traffic < over.traffic, 'queues hum less');
  const hidden = agents(...Array.from({ length: 30 }, (_, i) => [i, 0, -1, 0]));
  assert.equal(ambientMix(hidden, none, at(0, 0), NOON).traffic, 0, 'hidden agents are silent');
});

test('buildings and walkers soften birdsong without inventing traffic', () => {
  const city = ambientMix(agents([0, 0, 1, 1], [5, 5, 1, 1]), block(0, 0, 25, 30), at(0, 0), NOON);
  const fields = ambientMix(undefined, none, at(0, 0), NOON);
  assert.equal(city.traffic, 0, 'buildings and walkers never make engine sounds');
  assert.equal(fields.traffic, 0);
  assert.ok(fields.birds > 0.7, `open land: ${fields.birds}`);
  assert.ok(city.birds < fields.birds / 2, 'downtown drowns out birds');
  assert.equal(ambientMix(undefined, none, at(0, 0), MIDNIGHT).birds, 0, 'birds sleep at night');
});

test('zooming out never makes the same scene louder or brings distant cars into earshot', () => {
  const cars = new Float32Array([
    ...traffic(0, 0, 8),
    ...traffic(100, 0, 60),
    ...traffic(500, 0, 200),
  ]);
  let previous = { traffic: Infinity, birds: Infinity };
  for (const distance of [30, 100, 300, 600, 1200, 3000]) {
    const mix = ambientMix(cars, block(0, 0, 12, 6), at(0, 0, distance), NOON);
    assert.ok(mix.traffic < previous.traffic, `traffic falls at camera distance ${distance}`);
    assert.ok(mix.birds < previous.birds, `birdsong falls at camera distance ${distance}`);
    previous = mix;
    assert.equal(ambientMix(traffic(500, 0, 30), none, at(0, 0, distance), NOON).traffic, 0);
  }
  assert.ok(previous.traffic < 0.01, 'map-scale views have no minimum-volume floor');
});

test('a handful of cars is quiet and parked or hidden agents make no traffic sound', () => {
  const quiet = ambientMix(traffic(0, 0, 3), block(0, 0, 10, 6), at(0, 0), NOON);
  const busy = ambientMix(traffic(0, 0, 60), none, at(0, 0), NOON);
  assert.ok(quiet.traffic < 0.12, `small town traffic: ${quiet.traffic}`);
  assert.ok(quiet.traffic < busy.traffic / 4, 'sparse traffic does not saturate the mix');
  const parked = agents(...Array.from({ length: 30 }, (_, i) => [i, 0, 0, 0]));
  assert.equal(ambientMix(parked, block(0, 0, 20, 6), at(0, 0), NOON).traffic, 0);
});

test('daylight fades in at dawn and out at dusk', () => {
  const h = (hour) => daylight(hour * 3600);
  assert.equal(h(3), 0);
  assert.ok(h(5.75) > 0 && h(5.75) < 1);
  assert.equal(h(12), 1);
  assert.ok(h(20) > 0 && h(20) < 1);
  assert.equal(h(22), 0);
});
