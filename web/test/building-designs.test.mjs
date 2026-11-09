import assert from 'node:assert/strict';
import test from 'node:test';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { archetype, smokestack } from '../src/building-kit/kit.ts';

const rect = (x, z, w, d) => [
  [x, z],
  [x + w, z],
  [x + w, z + d],
  [x, z + d],
];
const record = (b) => [
  b.zone,
  b.height,
  b.roof,
  b.front,
  b.era ?? 1,
  b.ring.length,
  ...b.ring.flat(),
];
const box = (b) => {
  const g = buildBuildingGeometry(new Float32Array(record(b)));
  g.computeBoundingBox();
  return { g, ...g.boundingBox };
};

// Each use has its original look plus at least six new designs; industry's classic shed has
// four roof forms of its own.
const DESIGNS = {
  1: ['gable', 'hip', 'saltbox', 'gambrel', 'mansard', 'jerkinhead', 'porch', 'modern'],
  2: ['slab', 'mansard', 'terraced', 'brick', 'gallery', 'bays', 'gabled'],
  3: ['flat', 'hip', 'false-front', 'diner', 'gable-front', 'canopy', 'sign', 'cafe'],
  4: ['office', 'department', 'ribbon', 'podium', 'crown', 'stepped', 'signs'],
  5: ['classic', 'portal', 'monitor', 'arch', 'units', 'brick', 'silos'],
  6: ['classic', 'hall', 'tanks', 'cement', 'tower', 'stepped', 'domes'],
};
// Lot sizes and heights like the sim's for each use: [width, depth, height].
const SIZE = {
  1: [12, 11, 6],
  2: [18, 16, 15],
  3: [14, 12, 4],
  4: [24, 22, 24],
  5: [32, 24, 9],
  6: [44, 34, 12],
};

/** The first footprints (along x) that draw each design, by name. */
function samples(zone, shape = rect) {
  const [w, d, height] = SIZE[zone];
  const found = new Map();
  for (let k = 0; k < 400 && found.size < DESIGNS[zone].length; k++) {
    const b = {
      zone,
      height,
      roof: k % 4,
      front: 0,
      ring: shape(k * 100 + (k % 7) * 0.25, 0, w, d),
    };
    const name = archetype(b);
    if (!found.has(name)) found.set(name, b);
  }
  return found;
}

test('every use draws each of its designs, chosen by footprint', () => {
  for (const zone of [1, 2, 3, 4, 5, 6]) {
    const found = samples(zone);
    assert.deepEqual([...found.keys()].sort(), [...DESIGNS[zone]].sort(), `zone ${zone}`);
    for (const b of found.values()) assert.equal(archetype(b), archetype(structuredClone(b)));
  }
});

test('every design stands on its lot, reaches its sim height and looks its own', () => {
  for (const zone of [1, 2, 3, 4, 5, 6]) {
    const shapes = new Set();
    for (const [name, b] of samples(zone)) {
      const x0 = b.ring[0][0];
      const [w, d] = SIZE[zone];
      // Every front edge, so designs that build toward the street work facing any way.
      for (const front of [0, 1, 2, 3]) {
        const { g, min, max } = box({ ...b, front });
        const label = `zone ${zone} ${name} front ${front}`;
        assert.equal(min.y, 0, `${label} stands on the ground`);
        assert.ok(max.y >= b.height, `${label} reaches ${b.height} m`);
        assert.ok(max.y <= b.height + 26, `${label} is not absurdly tall`);
        // Overhangs, awnings, canopies and signs may reach 2 m past the walls.
        assert.ok(min.x >= x0 - 2 && max.x <= x0 + w + 2, `${label} fits across`);
        assert.ok(min.z >= -2 && max.z <= d + 2, `${label} fits in depth`);
        const p = g.getAttribute('position').array;
        assert.ok(p.every(Number.isFinite), `${label} is finite`);
        assert.equal(g.getAttribute('color').count, g.getAttribute('position').count);
        if (front === 0) shapes.add(JSON.stringify(normalised(p, x0)));
      }
    }
    assert.equal(shapes.size, DESIGNS[zone].length, `zone ${zone} designs differ`);
  }
});

/** A rough silhouette: positions moved to x = 0 and rounded, so colour choices do not count. */
const normalised = (p, x0) => [...p].map((v, i) => Math.round((i % 3 === 0 ? v - x0 : v) * 2));

test('designs keep working on L-shaped and narrow lots', () => {
  const ell = (x, z, w, d) => [
    [x, z],
    [x + w, z],
    [x + w, z + d * 0.5],
    [x + w * 0.5, z + d * 0.5],
    [x + w * 0.5, z + d],
    [x, z + d],
  ];
  const narrow = (x, z, w, d) => rect(x, z, Math.max(5, w * 0.35), d);
  for (const zone of [1, 2, 3, 4, 5, 6])
    for (const shape of [ell, narrow])
      for (const [name, b] of samples(zone, shape)) {
        const { g, min, max } = box(b);
        const label = `zone ${zone} ${name} on ${shape.name}`;
        assert.equal(min.y, 0, label);
        assert.ok(max.y >= b.height, label);
        assert.ok(g.getAttribute('position').array.every(Number.isFinite), label);
        const xs = b.ring.map((q) => q[0]);
        assert.ok(min.x >= Math.min(...xs) - 2 && max.x <= Math.max(...xs) + 2, `${label} fits`);
      }
});

test('a chimney stands inside its building and is the tallest thing it draws', () => {
  for (const zone of [5, 6])
    for (const [name, b] of samples(zone)) {
      const chimneyed = { ...b, roof: 3 };
      const stack = smokestack(chimneyed);
      assert.ok(stack, `zone ${zone} ${name} has a chimney on roof 3`);
      const [x, z] = stack.at;
      const [w, d] = SIZE[zone];
      const x0 = b.ring[0][0];
      assert.ok(x > x0 && x < x0 + w && z > 0 && z < d, `${name} chimney stands on the lot`);
      assert.ok(Math.abs(box(chimneyed).max.y - stack.top) < 0.01, `${name} smoke leaves the top`);
      assert.equal(smokestack({ ...b, roof: 0 }), null, `${name} has none on other roofs`);
    }
});
