import assert from 'node:assert/strict';
import test from 'node:test';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';

const rect = (x, z, w, d) => [
  [x, z],
  [x + w, z],
  [x + w, z + d],
  [x, z + d],
];
const record = (zone, height, roof, front, ring) => [
  zone,
  height,
  roof,
  front,
  ring.length,
  ...ring.flat(),
];
const build = (...records) => buildBuildingGeometry(new Float32Array(records.flat()));

test('an empty snapshot clears every building', () => {
  const g = build();
  assert.equal(g.getAttribute('position').count, 0);
});

test('every use stands on its footprint, reaches its sim height and stays near its lot', () => {
  for (let zone = 1; zone <= 6; zone++) {
    for (const roof of zone >= 5 ? [0, 1, 2, 3] : [0]) {
      const ring = rect(0, 0, zone >= 5 ? 30 : 14, zone >= 5 ? 22 : 12);
      const height = [6, 15, 8, 24, 9, 11][zone - 1];
      const g = build(record(zone, height, roof, 0, ring));
      const p = g.getAttribute('position');
      assert.equal(g.getAttribute('color').count, p.count);
      g.computeBoundingBox();
      const { min, max } = g.boundingBox;
      assert.equal(min.y, 0, `zone ${zone} stands on the ground`);
      assert.ok(max.y >= height, `zone ${zone} roof ${roof} reaches ${height} m`);
      // Overhangs, awnings and balconies may reach 2 m past the walls; nothing goes further.
      assert.ok(min.x >= -2 && min.z >= -2 && max.x <= 32 && max.z <= 24, `zone ${zone} fits`);
      for (let i = 0; i < p.count * 3; i++) assert.ok(Number.isFinite(p.array[i]));
    }
  }
});

test('flat roofs on concave lots do not bridge the gap', () => {
  const ring = [
    [0, 0],
    [20, 0],
    [20, 8],
    [8, 8],
    [8, 20],
    [0, 20],
  ];
  const g = build(record(4, 16, 0, 0, ring));
  const p = g.getAttribute('position');
  const n = g.getAttribute('normal');
  let roofArea = 0;
  for (let i = 0; i < p.count; i += 3) {
    if (n.getY(i) < 0.9 || p.getY(i) !== 16) continue;
    const x = (p.getX(i) + p.getX(i + 1) + p.getX(i + 2)) / 3;
    const z = (p.getZ(i) + p.getZ(i + 1) + p.getZ(i + 2)) / 3;
    assert.ok(x <= 8 || z <= 8, 'no roof across the concavity');
    roofArea +=
      Math.abs(
        (p.getX(i + 1) - p.getX(i)) * (p.getZ(i + 2) - p.getZ(i)) -
          (p.getZ(i + 1) - p.getZ(i)) * (p.getX(i + 2) - p.getX(i)),
      ) / 2;
  }
  assert.ok(roofArea >= 256, 'the whole footprint is roofed');
});

test('shopfronts and awnings face the sim front edge', () => {
  // Edge 0 runs along z = 0, edge 2 along z = 12; awnings reach out 1.7 m.
  for (const [front, side] of [
    [0, -1],
    [2, 1],
  ]) {
    const g = build(record(3, 4, 0, front, rect(0, 0, 16, 12)));
    g.computeBoundingBox();
    const { min, max } = g.boundingBox;
    if (side < 0) assert.ok(min.z < -1.5 && max.z <= 12.1, 'front 0 juts out toward -z');
    else assert.ok(max.z > 13.5 && min.z >= -0.1, 'front 2 juts out toward +z');
  }
});

test('industrial roofs follow the sim roof choice and each building is independent', () => {
  const records = [0, 1, 2, 3].map((roof) => record(5, 9, roof, 0, rect(roof * 40, 0, 30, 22)));
  const shapes = records.map((r) => [...build(r).getAttribute('position').array]);
  assert.equal(new Set(shapes.map((s) => s.join())).size, 4, 'four distinct roof forms');
  const all = [...build(...records).getAttribute('position').array];
  assert.deepEqual(all, shapes.flat(), 'packing order and neighbours do not change a building');
  assert.deepEqual(shapes[1], [...build(records[1]).getAttribute('position').array], 'stable');
});

test('growth rebuilds only the tiles whose buildings changed', async () => {
  const THREE = await import('three');
  const { BuildingLayer } = await import('../src/building-kit/layer.ts');
  const layer = new BuildingLayer(new THREE.MeshBasicMaterial());
  const near = record(1, 6, 0, 0, rect(10, 10, 12, 10));
  const far = record(1, 6, 0, 0, rect(2000, 10, 12, 10));
  layer.update(new Float32Array([...near, ...far]));
  assert.equal(layer.group.children.length, 2);
  const [a, b] = layer.group.children.map((m) => m.geometry);
  const grown = record(3, 4, 0, 0, rect(40, 10, 16, 12));
  layer.update(new Float32Array([...near, ...grown, ...far]));
  const after = layer.group.children.map((m) => m.geometry);
  assert.notEqual(after[0], a, 'the tile that grew is rebuilt');
  assert.equal(after[1], b, 'a distant tile keeps its geometry');
  layer.update(new Float32Array());
  assert.equal(layer.group.children.length, 0);
});
