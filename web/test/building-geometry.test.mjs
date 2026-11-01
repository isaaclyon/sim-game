import assert from 'node:assert/strict';
import test from 'node:test';
import { buildBuildingGeometry } from '../src/building-geometry.ts';

test('concave buildings have flat roofs at sim height and outward walls', () => {
  // An L-shaped building: the missing top-right quadrant must not get a roof.
  const ring = [
    [0, 0],
    [20, 0],
    [20, 8],
    [8, 8],
    [8, 20],
    [0, 20],
  ];
  const g = buildBuildingGeometry(new Float32Array([2, 18, ring.length, ...ring.flat()]));
  const p = g.getAttribute('position');
  const n = g.getAttribute('normal');
  const c = g.getAttribute('color');
  assert.equal(p.count, (ring.length - 2) * 3 + ring.length * 6);
  assert.equal(c.count, p.count);
  let area = 0;
  for (let i = 0; i < p.count; i += 3) {
    if (n.getY(i) > 0.9) {
      for (let j = i; j < i + 3; j++) assert.equal(p.getY(j), 18);
      const x = (p.getX(i) + p.getX(i + 1) + p.getX(i + 2)) / 3;
      const z = (p.getZ(i) + p.getZ(i + 1) + p.getZ(i + 2)) / 3;
      assert.ok(x <= 8 || z <= 8, 'no roof across the concavity');
      area +=
        Math.abs(
          (p.getX(i + 1) - p.getX(i)) * (p.getZ(i + 2) - p.getZ(i)) -
            (p.getZ(i + 1) - p.getZ(i)) * (p.getX(i + 2) - p.getX(i)),
        ) / 2;
    } else {
      assert.ok(Math.abs(n.getY(i)) < 1e-6);
      assert.ok([0, 18].includes(p.getY(i)));
    }
  }
  assert.equal(area, 256);
  // Bottom street edge faces outward, toward -z.
  const wall = (ring.length - 2) * 3;
  assert.equal(n.getZ(wall), -1);
  g.dispose();
});

test('six zones render together and an empty snapshot clears all buildings', () => {
  const data = [];
  for (let zone = 1; zone <= 6; zone++) {
    const x = zone * 30;
    data.push(zone, zone * 3, 4, x, 0, x + 10, 0, x + 10, 10, x, 10);
  }
  const g = buildBuildingGeometry(new Float32Array(data));
  assert.equal(g.getAttribute('position').count, 6 * 30);
  g.computeBoundingBox();
  assert.equal(g.boundingBox.max.y, 18);
  const colors = g.getAttribute('color');
  assert.notDeepEqual([...colors.array.slice(0, 3)], [...colors.array.slice(60 * 3, 60 * 3 + 3)]);
  const empty = buildBuildingGeometry(new Float32Array());
  assert.equal(empty.getAttribute('position').count, 0);
  g.dispose();
  empty.dispose();
});
