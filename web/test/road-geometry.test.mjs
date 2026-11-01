import assert from 'node:assert/strict';
import test from 'node:test';
import { buildRoadGeometry } from '../src/road-geometry.ts';

function geometry(roads) {
  return buildRoadGeometry(new Float32Array(roads.flat()));
}

function covers(mesh, x, z) {
  const p = mesh.getAttribute('position');
  for (let i = 0; i < p.count; i += 3) {
    const sides = [0, 1, 2].map((j) => {
      const a = i + j;
      const b = i + ((j + 1) % 3);
      return (p.getX(b) - p.getX(a)) * (z - p.getZ(a)) - (p.getZ(b) - p.getZ(a)) * (x - p.getX(a));
    });
    if (sides.every((s) => s <= 1e-5)) return true;
  }
  return false;
}

test('ribbons have eight-metre width in any direction and face up', () => {
  for (const [x, z] of [
    [100, 0],
    [0, 100],
    [60, 80],
    [-60, -80],
  ]) {
    const mesh = geometry([[0, 0, x, z, 10]]);
    const p = mesh.getAttribute('position');
    assert.equal(p.count, 6);
    for (let i = 0; i < p.count; i++) {
      assert.equal(p.getY(i), 0);
      assert.ok(Math.abs(Math.abs((x * p.getZ(i) - z * p.getX(i)) / 100) - 4) < 1e-5);
    }
    for (let i = 0; i < p.count; i += 3) {
      const crossY =
        (p.getZ(i + 1) - p.getZ(i)) * (p.getX(i + 2) - p.getX(i)) -
        (p.getX(i + 1) - p.getX(i)) * (p.getZ(i + 2) - p.getZ(i));
      assert.ok(crossY > 0);
    }
    assert.ok(covers(mesh, x / 2, z / 2));
    assert.ok(!covers(mesh, -x / 10, -z / 10));
    mesh.dispose();
  }
});

test('bends, T-junctions and crossings get a gap-free pad only at the shared endpoint', () => {
  for (const ends of [
    [
      [100, 0],
      [0, 100],
    ],
    [
      [60, 80],
      [80, 60],
    ],
    [
      [100, 0],
      [-100, 0],
      [0, 100],
    ],
    [
      [100, 0],
      [-100, 0],
      [0, 100],
      [0, -100],
    ],
  ]) {
    // Alternate endpoint order, as the sim may emit a join at either end of a road.
    const mesh = geometry(ends.map(([x, z], i) => (i % 2 ? [x, z, 0, 0, 10] : [0, 0, x, z, 10])));
    assert.equal(mesh.getAttribute('position').count, ends.length * 6 + 12 * 3);
    for (let i = 0; i < 360; i++) {
      const angle = (i * Math.PI) / 180;
      assert.ok(covers(mesh, 3.99 * Math.cos(angle), 3.99 * Math.sin(angle)));
    }
    mesh.dispose();
  }
});

test('speed limits do not change asphalt geometry', () => {
  const street = geometry([[0, 0, 100, 0, 10]]);
  const avenue = geometry([[0, 0, 100, 0, 25]]);
  assert.deepEqual(street.getAttribute('position').array, avenue.getAttribute('position').array);
});

test('empty and zero-length roads produce no invalid triangles', () => {
  for (const roads of [[], [[5, 5, 5, 5, 10]]]) {
    assert.equal(geometry(roads).getAttribute('position').count, 0);
  }
});

test('replacement snapshots do not retain old pads', () => {
  const junction = geometry([
    [0, 0, 100, 0, 10],
    [0, 0, 0, 100, 10],
  ]);
  const replacement = geometry([[0, 0, 100, 0, 10]]);
  assert.ok(covers(junction, -2, -2));
  assert.ok(!covers(replacement, -2, -2));
});
