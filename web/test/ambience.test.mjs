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
  assert.ok(over.traffic > 0.5, `busy road: ${over.traffic}`);
  assert.equal(away.traffic, 0, 'cars out of earshot are silent');
  const stopped = agents(...Array.from({ length: 30 }, (_, i) => [i, 0, 0, 0]));
  assert.ok(ambientMix(stopped, none, at(0, 0), NOON).traffic < over.traffic, 'queues hum less');
  const hidden = agents(...Array.from({ length: 30 }, (_, i) => [i, 0, -1, 0]));
  assert.equal(ambientMix(hidden, none, at(0, 0), NOON).traffic, 0, 'hidden agents are silent');
});

test('buildings and walkers make city noise; open land has birds by day', () => {
  const city = ambientMix(agents([0, 0, 1, 1], [5, 5, 1, 1]), block(0, 0, 25, 30), at(0, 0), NOON);
  const fields = ambientMix(undefined, none, at(0, 0), NOON);
  assert.ok(city.city > 0.5, `downtown: ${city.city}`);
  assert.equal(fields.city, 0);
  assert.ok(fields.birds > 0.7, `open land: ${fields.birds}`);
  assert.ok(city.birds < fields.birds / 2, 'downtown drowns out birds');
  assert.equal(ambientMix(undefined, none, at(0, 0), MIDNIGHT).birds, 0, 'birds sleep at night');
});

test('zooming out widens what is heard but softens the detail', () => {
  const cars = traffic(500, 0, 30);
  assert.equal(ambientMix(cars, none, at(0, 0, 50), NOON).traffic, 0, 'close up: far cars silent');
  assert.ok(ambientMix(cars, none, at(0, 0, 1200), NOON).traffic > 0, 'high up: they join in');
  const birds = (d) => ambientMix(undefined, none, at(0, 0, d), NOON).birds;
  assert.ok(birds(2000) < birds(100) / 2, 'birds are a close-up sound');
});

test('daylight fades in at dawn and out at dusk', () => {
  const h = (hour) => daylight(hour * 3600);
  assert.equal(h(3), 0);
  assert.ok(h(5.75) > 0 && h(5.75) < 1);
  assert.equal(h(12), 1);
  assert.ok(h(20) > 0 && h(20) < 1);
  assert.equal(h(22), 0);
});
