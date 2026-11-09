import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { RISE_SECONDS } from '../src/building-kit/layer.ts';
import { COAL_CHIMNEY, createCoalPlant } from '../src/coal-plant.ts';
import { MAX_STACKS, PUFF_LIFE, puffAt, SmokeLayer } from '../src/smoke-layer.ts';

const rect = (x, z, w, d) => [x, z, x + w, z, x + w, z + d, x, z + d];
const record = (zone, roof, x) => [zone, 9, roof, 0, 1, 4, ...rect(x, 0, 30, 22)];
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

// [kind, x, y, half, front_x, front_y, terminal_x, terminal_y, output, grid] per plant.
const plantRecord = (kind, x, y, fx, fy) => [kind, x, y, 22, fx, fy, 0, 0, 3000, 0];

test('coal plants smoke from their chimney; solar and wind do not', () => {
  const layer = new SmokeLayer();
  layer.setPlants(
    new Float32Array([...plantRecord(1, 0, 0, 0, 1), ...plantRecord(2, 50, 0, 0, 1)]),
  );
  layer.animate(100, [0, 0]);
  assert.equal(puffs(layer).count, 0);

  layer.setPlants(new Float32Array(plantRecord(0, 100, 200, 0, 1)));
  layer.animate(100, [0, 0]);
  const [stack] = layer.shown;
  assert.ok(puffs(layer).count > 16, 'a coal plant puffs more than any factory');
  assert.deepEqual(
    [stack.x, stack.z].map((v) => Math.round(v * 10) / 10),
    [113.4, 191.5],
  );
  assert.ok(stack.top > 42 && stack.top < 43, 'smoke leaves the drawn stack top');
});

test('coal smoke follows the plant turned to face its road', () => {
  for (const [fx, fy, half] of [
    [1, 0, 22],
    [0.6, -0.8, 22],
    [-0.28, 0.96, 30],
  ]) {
    const layer = new SmokeLayer();
    layer.setPlants(new Float32Array([0, 100, 200, half, fx, fy, 0, 0, 3000, 0]));
    const [stack] = layer.plantStacks;
    // Placed as power-layer.ts places the model.
    const plant = new THREE.Group();
    plant.position.set(100, 0, 200);
    plant.rotation.y = Math.atan2(fx, fy);
    const model = createCoalPlant(half);
    plant.add(model);
    plant.updateMatrixWorld(true);
    const mouth = model.localToWorld(new THREE.Vector3(COAL_CHIMNEY.x, 0, COAL_CHIMNEY.z));
    assert.ok(Math.hypot(stack.x - mouth.x, stack.z - mouth.z) < 1e-3, `${fx}, ${fy}`);
  }
});

test('factory and coal smoke draw together', () => {
  const layer = new SmokeLayer();
  layer.update(snapshot(record(5, 3, 0)), new Uint32Array([1]));
  layer.setPlants(new Float32Array(plantRecord(0, 100, 200, 0, 1)));
  layer.animate(100, [0, 0]);
  assert.equal(layer.shown.length, 2);
});

test('smoke draws after the roads beneath it', () => {
  const layer = new SmokeLayer();
  layer.update(snapshot(record(5, 3, 0)), new Uint32Array([1]));
  layer.animate(100, [0, 0]);
  // Lane paint, the last flat ground layer, draws at 5 (renderer.ts).
  assert.ok(puffs(layer).renderOrder > 5);
});

test('coal plumes fan out toward the top; factory plumes keep their shape', () => {
  const base = { x: 0, z: 0, top: 20, size: 1, puffs: 6, phase: 0, from: 0 };
  const at = (stack, age) => puffAt(stack, 0, 10 * PUFF_LIFE + age);
  const coal = { ...base, spread: 2.5 };
  const young = PUFF_LIFE * 0.05;
  assert.ok(at(coal, young)[3] / at(base, young)[3] < 1.1, 'the base stays narrow');
  const old = PUFF_LIFE * 0.7;
  assert.ok(at(coal, old)[3] > at(base, old)[3] * 1.4, 'the top is wider');
  assert.deepEqual(at({ ...base, spread: 1 }, old), at(base, old));
});
