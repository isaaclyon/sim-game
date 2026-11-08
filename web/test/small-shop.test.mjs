import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { PRESENT_DAY } from '../src/building-kit/style.ts';

const record = (x, zone = 3) => [zone, 4, 0, 0, 4, x, 0, x + 13, 0, x + 13, 14, x, 14];
const near = (c, i, col) =>
  Math.abs(c.getX(i) - col.r) + Math.abs(c.getY(i) - col.g) + Math.abs(c.getZ(i) - col.b) < 1e-4;

test('small shops have individual front doors and no oversized rooftop machinery', () => {
  for (let x = 0; x < 240; x += 30) {
    const g = buildBuildingGeometry(new Float32Array(record(x))).toNonIndexed();
    const p = g.getAttribute('position');
    const c = g.getAttribute('color');
    const door = new THREE.Color(PRESENT_DAY.door);
    const plant = new THREE.Color(PRESENT_DAY.rooftop);
    const glass = new THREE.Color(PRESENT_DAY.shopGlass);
    let doorVertices = 0;
    for (let i = 0; i < p.count; i++) {
      if (near(c, i, door)) {
        doorVertices++;
        assert.ok(p.getZ(i) < 0, 'the door faces the frontage road');
        assert.ok(p.getY(i) < 3, 'the door belongs to the trading floor');
      }
      assert.ok(!near(c, i, plant), 'small shops do not look like plant-covered warehouse roofs');
    }
    for (let i = 0; i < p.count; i += 3) {
      if (!near(c, i, glass) || p.getZ(i) >= 0) continue;
      const xs = [p.getX(i), p.getX(i + 1), p.getX(i + 2)];
      const door = x + 13 * 0.3;
      assert.ok(
        Math.max(...xs) < door - 0.5 || Math.min(...xs) > door + 0.5,
        'shop glazing never covers the door',
      );
    }
    assert.ok(doorVertices >= 6, 'a distinct entrance breaks up the shop window');
  }
});

test('small-shop roof forms vary without changing sim footprints or heights', () => {
  const sloped = [];
  for (let x = 0; x < 240; x += 30) {
    const g = buildBuildingGeometry(new Float32Array(record(x))).toNonIndexed();
    const n = g.getAttribute('normal');
    const p = g.getAttribute('position');
    sloped.push(
      Array.from({ length: n.count }, (_, i) => i).some(
        (i) => p.getY(i) > 4.1 && n.getY(i) > 0.2 && n.getY(i) < 0.99,
      ),
    );
    g.computeBoundingBox();
    assert.equal(g.boundingBox.min.y, 0);
    assert.ok(g.boundingBox.max.y >= 4 && g.boundingBox.max.y < 7);
  }
  assert.ok(sloped.some(Boolean), 'some shops have shallow pitched roofs');
  assert.ok(
    sloped.some((v) => !v),
    'some shops retain simple flat roofs',
  );
});
