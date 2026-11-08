import assert from 'node:assert/strict';
import test from 'node:test';
import { MeshBuilder } from '../src/building-kit/mesh.ts';
import { inside, offset } from '../src/building-kit/polygon.ts';
import { boundary } from '../src/lot-scenery/boundary.ts';
import { fits, fitter } from '../src/lot-scenery/placement.ts';
import { Clearance } from '../src/scenery/clearance.ts';

/** Repeatable random numbers in [0, 1). */
const random = (seed) => () => {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 2 ** 32;
};
const mix = (a, b, t) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
/** A counter-clockwise ring around `c`, concave when its radii vary. */
const star = (rand, c, n, r0, r1) =>
  Array.from({ length: n }, (_, i) => {
    const a = ((i + rand() * 0.6) / n) * Math.PI * 2;
    const r = r0 + rand() * (r1 - r0);
    return [c[0] + Math.cos(a) * r, c[1] + Math.sin(a) * r];
  });
const roadsAround = (rand, n) => {
  const c = new Clearance();
  const segments = [];
  for (let i = 0; i < n; i++) {
    const a = [rand() * 80 - 40, rand() * 80 - 40];
    segments.push(...a, a[0] + rand() * 30 - 15, a[1] + rand() * 30 - 15, Math.floor(rand() * 3));
  }
  c.setRoads(new Float32Array(segments));
  return c;
};

/** The direct check `fitter` must agree with: every lot side, ring and road sample. */
function reference(shape, lot, occupied, roads) {
  const cross = (a, b, c) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  const crosses = (a, b, c, d) =>
    cross(a, b, c) * cross(a, b, d) < -1e-8 && cross(c, d, a) * cross(c, d, b) < -1e-8;
  const intersects = (a, b) =>
    a.some((p, i) => b.some((q, j) => crosses(p, a[(i + 1) % a.length], q, b[(j + 1) % b.length])));
  if (!shape.every((p) => inside(p, lot)) || intersects(shape, lot)) return false;
  for (const ring of occupied)
    if (
      shape.some((p) => inside(p, ring)) ||
      ring.some((p) => inside(p, shape)) ||
      intersects(shape, ring)
    )
      return false;
  for (let i = 0; i < shape.length; i++) {
    const [a, b] = [shape[i], shape[(i + 1) % shape.length]];
    const steps = Math.max(1, Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) * 4));
    for (let j = 0; j <= steps; j++) {
      const p = mix(a, b, j / steps);
      if (roads.roadGap(p[0], p[1]) < 0.175) return false;
    }
  }
  return true;
}

test('a fitter answers as the direct check for every piece of its region', () => {
  const rand = random(7);
  let [yes, no] = [0, 0];
  for (let round = 0; round < 60; round++) {
    const lot = star(rand, [0, 0], 24, 14, 30);
    const occupied = [
      star(rand, [rand() * 10, rand() * 10], 14, 4, 12),
      star(rand, [-8, 4], 4, 2, 5),
    ];
    const roads = roadsAround(rand, 6);
    // A thin strip like a boundary's, cut into pieces at random points along it.
    const [a, b] = [star(rand, [0, 0], 1, 5, 28)[0], star(rand, [0, 0], 1, 5, 28)[0]];
    const width = 0.2 + rand();
    const n = [-(b[1] - a[1]), b[0] - a[0]].map(
      (v) => (v / Math.hypot(b[0] - a[0], b[1] - a[1])) * width,
    );
    const strip = (s, t) => [
      mix(a, b, s),
      mix(a, b, t),
      [...mix(a, b, t)].map((v, k) => v + n[k]),
      [...mix(a, b, s)].map((v, k) => v + n[k]),
    ];
    const check = fitter(strip(0, 1), lot, occupied, roads);
    for (let k = 0; k < 40; k++) {
      const [s, t] = [rand(), rand()].sort();
      const piece = strip(s, t);
      const want = reference(piece, lot, occupied, roads);
      assert.equal(check(piece), want, `round ${round}, piece ${k}`);
      assert.equal(fits(piece, lot, occupied, roads), want);
      if (want) yes++;
      else no++;
    }
  }
  assert.ok(yes > 100 && no > 100, `both answers covered: ${yes} fit, ${no} do not`);
});

test('nearby road gaps match roadGap below their margin; held lines stay on one road', () => {
  const rand = random(11);
  const roads = roadsAround(rand, 40);
  for (let round = 0; round < 200; round++) {
    const [x, y] = [rand() * 80 - 40, rand() * 80 - 40];
    const box = [x, y, x + rand() * 6, y + rand() * 2];
    const gap = roads.gapsNear(box, 0.5);
    for (let k = 0; k < 20; k++) {
      const p = [box[0] + rand() * (box[2] - box[0]), box[1] + rand() * (box[3] - box[1])];
      const full = roads.roadGap(p[0], p[1]);
      if (full < 0.5) assert.equal(gap?.(p[0], p[1]), full);
      else if (gap) assert.ok(gap(p[0], p[1]) >= 0.5);
    }
    const [p, q] = [
      [box[0], box[1]],
      [box[2], box[3]],
    ];
    if (roads.holds(p, q, 0.5))
      for (let t = 0; t <= 1; t += 0.05) assert.ok(roads.roadGap(...mix(p, q, t)) < 0.5);
  }
});

/** Boundary runs from halving every piece that does not fit down to 0.2 m, never stopping early. */
function plainRuns(lot, obstacles, roads) {
  const [outer, inner] = [offset(lot, -0.2), offset(lot, -0.65)];
  const runs = [];
  for (let i = 0; i < lot.length; i++) {
    const j = (i + 1) % lot.length;
    const length = Math.hypot(lot[j][0] - lot[i][0], lot[j][1] - lot[i][1]);
    const strip = (a, b) => [
      mix(outer[i], outer[j], a),
      mix(outer[i], outer[j], b),
      mix(inner[i], inner[j], b),
      mix(inner[i], inner[j], a),
    ];
    const edge = [];
    const cut = (a, b) => {
      if (fits(strip(a, b), lot, obstacles, roads)) {
        const last = edge.at(-1);
        if (last && Math.abs(last[1] - a) < 1e-8) last[1] = b;
        else edge.push([a, b]);
      } else if ((b - a) * length >= 0.2) {
        cut(a, (a + b) / 2);
        cut((a + b) / 2, b);
      }
    };
    cut(0, 1);
    runs.push(...edge.map(([a, b]) => strip(a, b)));
  }
  return runs;
}

test('boundaries stop cutting early only where no smaller piece could fit', () => {
  const rand = random(3);
  let pieces = 0;
  for (let round = 0; round < 30; round++) {
    const lot = star(rand, [0, 0], 6 + Math.floor(rand() * 10), 18, 24);
    // Buildings and paths reaching over the boundary, concave or not, and roads across it.
    const obstacles = [star(rand, [rand() * 6, rand() * 6], 4 + Math.floor(rand() * 14), 12, 22)];
    if (round % 2) obstacles.push(star(rand, [20, 0], 4, 3, 6));
    const roads = roadsAround(rand, 3);
    const got = boundary(new MeshBuilder(), lot, obstacles, roads, 'hedge');
    assert.deepEqual(got, plainRuns(lot, obstacles, roads), `round ${round}`);
    pieces += got.length;
  }
  assert.ok(pieces > 30, `${pieces} boundary pieces`);
});

test('a boundary passing out of a concave obstacle through its corners is still built there', () => {
  const lot = [
    [0, 0],
    [40, 0],
    [40, 30],
    [0, 30],
  ];
  // The rear strip's outer side runs along y = 29.8, with both ends inside this obstacle. A
  // diamond-shaped notch, open to the south, has its west and east corners exactly on that
  // line, so the side leaves the obstacle through one corner and returns through the other.
  const obstacle = [
    [-5, 25],
    [19, 25],
    [19, 27.5],
    [15, 29.8],
    [20, 33],
    [25, 29.8],
    [21, 27.5],
    [21, 25],
    [45, 25],
    [45, 35],
    [-5, 35],
  ];
  const got = boundary(new MeshBuilder(), lot, [obstacle], new Clearance(), 'hedge');
  assert.deepEqual(got, plainRuns(lot, [obstacle], new Clearance()));
  assert.ok(got.length > 0, 'the notch gets its piece of boundary');
});
