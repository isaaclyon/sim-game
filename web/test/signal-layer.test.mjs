import assert from 'node:assert/strict';
import test from 'node:test';
import { LAMPS, litLamps, POLE } from '../src/signal-layer.ts';

const bounds = (g) => {
  g.computeBoundingBox();
  return g.boundingBox;
};

test('each state lights the lamps drivers expect', () => {
  assert.deepEqual(litLamps(0), ['red']);
  assert.deepEqual(litLamps(1), ['green']);
  assert.deepEqual(litLamps(2), ['green', 'arrow']);
  assert.deepEqual(litLamps(7), ['red'], 'an unknown state shows red');
});

test('the pole stands on the kerb and its head hangs over the approach lane', () => {
  const pole = bounds(POLE());
  assert.equal(pole.min.y, 0);
  assert.ok(pole.max.y > 5 && pole.max.y < 7.5, 'about six metres tall');
  // The signal sits 6 m right of the centreline; the lane centre is 2 m right, so 4 m left.
  const red = bounds(LAMPS.red.geometry());
  assert.ok(Math.abs((red.min.z + red.max.z) / 2 - 4) < 0.6, 'over the lane');
  assert.ok(red.max.x < 0, 'lamps face oncoming cars, which come from -x');
  const order = ['red', 'amber', 'green'].map((k) => bounds(LAMPS[k].geometry()).min.y);
  assert.ok(order[0] > order[1] && order[1] > order[2], 'red on top, green at the bottom');
  const arrow = bounds(LAMPS.arrow.geometry());
  assert.ok(arrow.min.z > red.max.z, 'the turn arrow is on the driver’s left');
});
