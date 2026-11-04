import assert from 'node:assert/strict';
import test from 'node:test';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { RISE_SECONDS } from '../src/building-kit/layer.ts';
import { MAX_STACKS, PUFF_LIFE, puffAt, SmokeLayer } from '../src/smoke-layer.ts';

const rect = (x, z, w, d) => [x, z, x + w, z, x + w, z + d, x, z + d];
const record = (zone, roof, x) => [zone, 9, roof, 0, 4, ...rect(x, 0, 30, 22)];
const snapshot = (...records) => new Float32Array(records.flat());
const puffs = (layer) => layer.group.children[0];

test('only industrial buildings with a drawn chimney smoke', () => {
  const layer = new SmokeLayer();
  const data = snapshot(record(5, 3, 0), record(5, 0, 40), record(2, 3, 80), record(6, 3, 120));
  layer.update(data, new Uint32Array([1, 2, 3, 4]));
  layer.animate(100, [0, 0]);
  assert.equal(puffs(layer).count, 11 + 16, 'high-density industry puffs more');
});

test('smoke leaves the top of the chimney the kit draws', () => {
  const data = snapshot(record(5, 3, 0));
  const layer = new SmokeLayer();
  layer.update(data, new Uint32Array([7]));
  const stack = layer.stacks.get(7);
  const pos = buildBuildingGeometry(data).getAttribute('position');
  let top = 0;
  for (let i = 0; i < pos.count; i++) top = Math.max(top, pos.getY(i));
  assert.ok(Math.abs(stack.top - top) < 1e-3, `stack top ${stack.top} vs drawn ${top}`);
  const near = [];
  for (let i = 0; i < pos.count; i++)
    if (pos.getY(i) > top - 1e-3)
      near.push(Math.hypot(pos.getX(i) - stack.x, pos.getZ(i) - stack.z));
  assert.ok(Math.max(...near) < 1.01, 'the top ring is around the smoke source');
});

test('puffs rise, drift, grow and fade over their life', () => {
  const stack = { x: 0, z: 0, top: 20, size: 1, puffs: 6, phase: 0, from: 0 };
  const at = (age) => puffAt(stack, 0, 10 * PUFF_LIFE + age);
  assert.ok(at(0)[3] < 0.01, 'a puff starts small');
  assert.ok(at(PUFF_LIFE * 0.4)[3] > 1.5, 'and grows');
  assert.ok(at(PUFF_LIFE * 0.999)[3] < 0.05, 'and fades out');
  assert.ok(at(3)[1] > at(1)[1], 'it rises');
  assert.ok(at(5)[0] > at(1)[0], 'it drifts downwind');
});

test('a new factory smokes only once it has risen', () => {
  const layer = new SmokeLayer();
  layer.update(snapshot(record(5, 3, 0)), new Uint32Array([1]), 50);
  const visible = (now) => {
    layer.animate(now, [0, 0]);
    const m = puffs(layer);
    const e = m.instanceMatrix.array;
    return Array.from({ length: m.count }, (_, i) => e[16 * i]).some((s) => s > 1e-6);
  };
  assert.equal(visible(50 + RISE_SECONDS - 0.1), false);
  assert.equal(visible(50 + RISE_SECONDS + 2), true);
  // A city's first snapshot smokes at once.
  layer.update(snapshot(record(5, 3, 0)), new Uint32Array([2]));
  assert.equal(visible(10), true);
});

test('large cities draw only the chimneys nearest the camera', () => {
  const records = Array.from({ length: MAX_STACKS + 50 }, (_, i) => record(5, 3, i * 40));
  const layer = new SmokeLayer();
  layer.update(
    snapshot(...records),
    Uint32Array.from(records, (_, i) => i),
  );
  layer.animate(100, [0, 0]);
  assert.equal(puffs(layer).count, MAX_STACKS * 11);
  const xs = layer.shown.map((s) => s.x);
  assert.ok(Math.max(...xs) < MAX_STACKS * 40 + 40, 'the far ones are dropped');
});

test('smoke stops glowing at night', () => {
  const night = { value: 0 };
  const layer = new SmokeLayer(night);
  layer.update(snapshot(record(5, 3, 0)), new Uint32Array([1]));
  layer.animate(100, [0, 0]);
  assert.ok(puffs(layer).material.emissive.r > 0.05);
  night.value = 1;
  layer.animate(100, [0, 0]);
  assert.equal(puffs(layer).material.emissive.r, 0);
});
