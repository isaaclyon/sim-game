import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { Lighting } from '../src/lighting.ts';
import { createSimScene } from '../src/scene.ts';
import { LAMP_POST, LIGHT_POOL, placeStreetlights } from '../src/streetlights.ts';

const STREET = 0;
const MAIN = 1;
const ALLEY = 2;
const lamps = (segments) => {
  const out = placeStreetlights(new Float32Array(segments));
  return Array.from({ length: out.length / 3 }, (_, i) => [...out.subarray(i * 3, i * 3 + 3)]);
};

test('a street gets a lamp every 30 m, alternating sides on the sidewalk, facing the road', () => {
  const placed = lamps([0, 0, 200, 0, STREET]);
  assert.deepEqual(
    placed.map(([x]) => Math.round(x)),
    [30, 60, 90, 120, 150, 180],
  );
  for (const [i, [, y, heading]] of placed.entries()) {
    assert.ok(Math.abs(Math.abs(y) - 4.4) < 1e-4, 'just past the 4 m asphalt edge');
    if (i > 0) assert.equal(Math.sign(y), -Math.sign(placed[i - 1][1]), 'sides alternate');
    // The arm reaches from the pole toward the centreline.
    assert.ok(Math.abs(Math.sin(heading) + Math.sign(y)) < 1e-6);
    assert.ok(Math.abs(Math.cos(heading)) < 1e-6);
  }
});

test('main roads are lit more densely than streets, alleys more sparsely', () => {
  const count = (cls) => lamps([0, 0, 300, 0, cls]).length;
  assert.ok(count(MAIN) > count(STREET));
  assert.ok(count(ALLEY) < count(STREET));
  for (const [, y] of lamps([0, 0, 300, 0, MAIN])) assert.ok(Math.abs(Math.abs(y) - 6.4) < 1e-4);
});

test('lamps keep clear of junctions and never stand on another road', () => {
  const placed = lamps(
    [
      [-200, 0, 0, 0, STREET],
      [0, 0, 200, 0, STREET],
      [0, -200, 0, 0, STREET],
      [0, 0, 0, 200, STREET],
    ].flat(),
  );
  assert.ok(placed.length > 20);
  for (const [x, y] of placed) {
    assert.ok(Math.hypot(x, y) >= 12, `lamp at ${x}, ${y} crowds the junction`);
    assert.ok(Math.min(Math.abs(x), Math.abs(y)) > 4, 'not on asphalt');
  }
});

test('spacing carries across the samples of a curve', () => {
  // A quarter circle of radius 100 in 16 samples; arc length about 157 m.
  const segments = [];
  for (let i = 0; i < 16; i++) {
    const [a, b] = [(i / 16) * (Math.PI / 2), ((i + 1) / 16) * (Math.PI / 2)];
    segments.push(100 * Math.cos(a), 100 * Math.sin(a), 100 * Math.cos(b), 100 * Math.sin(b), 0);
  }
  const placed = lamps(segments);
  assert.equal(placed.length, 5, 'one lamp per 30 m of arc, not per sample');
});

test('the post has a glowing lens over the road and the pool fades to nothing at its rim', () => {
  const post = LAMP_POST();
  post.computeBoundingBox();
  assert.equal(post.boundingBox.min.y, 0);
  assert.ok(post.boundingBox.max.y > 6 && post.boundingBox.max.y < 9);
  const [pos, glow] = [post.getAttribute('position'), post.getAttribute('nightEmission')];
  let lit = 0;
  for (let i = 0; i < glow.count; i++)
    if (glow.getX(i) > 0) {
      lit++;
      assert.ok(pos.getX(i) > 1, 'only the lens, at the end of the arm, glows');
    }
  assert.ok(lit > 0);
  const pool = LIGHT_POOL();
  const [pp, pc] = [pool.getAttribute('position'), pool.getAttribute('color')];
  let rim = 0;
  for (let i = 0; i < pp.count; i++) {
    const r = Math.hypot(pp.getX(i) - 1.8, pp.getZ(i));
    rim = Math.max(rim, r);
    if (r > 13.9) assert.equal(pc.getX(i), 0);
    if (r < 0.01) assert.ok(pc.getX(i) > 0.5);
  }
  assert.ok(Math.abs(rim - 14) < 0.01, 'light spreads 14 m from under the lens');
});

test('streetlights are off by day, switch on at dusk and stay on through the night', () => {
  const scene = createSimScene();
  const lighting = new Lighting(scene);
  const at = (hour) => {
    lighting.setTime(hour * 3600);
    lighting.update(scene, new THREE.Vector3(), 300);
    return lighting.streetlights.value;
  };
  assert.equal(at(12), 0);
  assert.equal(at(17), 0);
  assert.equal(at(19), 1);
  assert.equal(at(0), 1);
  assert.equal(at(4.9), 1);
  assert.equal(at(7), 0);
});
