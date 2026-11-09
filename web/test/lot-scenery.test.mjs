import assert from 'node:assert/strict';
import test from 'node:test';
import { MeshBuilder } from '../src/building-kit/mesh.ts';
import { inside } from '../src/building-kit/polygon.ts';
import { boundary } from '../src/lot-scenery/boundary.ts';
import { lotGeometry } from '../src/lot-scenery/geometry.ts';
import { LotLayer } from '../src/lot-scenery/layer.ts';
import { fits } from '../src/lot-scenery/placement.ts';
import { Clearance } from '../src/scenery/clearance.ts';

const rect = (x, y, w, d) => [
  [x, y],
  [x + w, y],
  [x + w, y + d],
  [x, y + d],
];
const lot = rect(0, 0, 28, 40);
const building = (zone = 1) => ({ zone, height: 6, roof: 0, front: 0, ring: rect(7, 6, 14, 14) });
const record = (b) => [
  b.zone,
  b.height,
  b.roof,
  b.front,
  b.era ?? 1,
  b.ring.length,
  ...b.ring.flat(),
];
const packedLots = (...rings) => new Float32Array(rings.flatMap((r) => [r.length, ...r.flat()]));
const packedBuildings = (...bs) => new Float32Array(bs.flatMap(record));
const buildLotGeometry = (lot, b, roads, ground = true) => {
  const mb = new MeshBuilder();
  lotGeometry(mb, lot, b, roads, ground ? mb : new MeshBuilder());
  return mb.geometry();
};

test('outdoor geometry is stable, varied by use, and stays inside its lot', () => {
  const signatures = new Set();
  for (let zone = 1; zone <= 6; zone++) {
    const b = building(zone);
    const g = buildLotGeometry(lot, b, new Clearance());
    const p = g.getAttribute('position');
    assert.ok(p.count > 6, 'surface plus outdoor details');
    for (let i = 0; i < p.count; i++) {
      assert.ok(Number.isFinite(p.getY(i)));
      assert.ok(p.getX(i) >= 0 && p.getX(i) <= 28);
      assert.ok(p.getZ(i) >= 0 && p.getZ(i) <= 40);
    }
    const again = buildLotGeometry(lot, b, new Clearance());
    const vertices = (geometry) => [
      ...geometry.getAttribute('position').array,
      ...geometry.getAttribute('color').array,
    ];
    assert.deepEqual(vertices(g), vertices(again));
    signatures.add(vertices(g).join(','));
  }
  assert.ok(signatures.size >= 3, 'homes, commerce and industry have distinct treatments');
});

test('small shops leave planted rear space instead of paving the entire property', () => {
  let trees = 0;
  for (let i = 0; i < 16; i++) {
    const shift = (ring) => ring.map(([x, z]) => [x + i * 40, z]);
    const site = shift(lot);
    const small = buildLotGeometry(
      site,
      { ...building(3), ring: shift(building().ring) },
      new Clearance(),
    );
    const dense = buildLotGeometry(
      site,
      { ...building(4), ring: shift(building().ring) },
      new Clearance(),
    );
    const grass = small.getAttribute('color');
    const paved = dense.getAttribute('color');
    assert.ok(grass.getY(0) > grass.getX(0) && grass.getY(0) > grass.getZ(0), 'green ground');
    assert.ok(Math.abs(paved.getX(0) - paved.getY(0)) < 0.2, 'dense commerce is paved, not green');
    small.computeBoundingBox();
    dense.computeBoundingBox();
    if (small.boundingBox.max.y > 3) trees++;
    assert.ok(dense.boundingBox.max.y < 2, 'dense lots keep low details');
  }
  assert.ok(trees > 0, 'some rear spaces accommodate a small tree');
});

test('placement rejects concave crossings, building overlaps and road intrusions', () => {
  const land = [
    [0, 0],
    [20, 0],
    [20, 20],
    [12, 20],
    [12, 8],
    [8, 8],
    [8, 20],
    [0, 20],
  ];
  const c = new Clearance();
  assert.equal(fits(rect(5, 10, 10, 2), land, [], c), false, 'corners alone miss the notch');
  assert.equal(fits(rect(1, 1, 4, 4), land, [rect(2, 2, 1, 1)], c), false);
  c.setRoads(new Float32Array([0, 0, 20, 0, 1]));
  assert.equal(fits(rect(2, 2, 3, 2), land, [], c), false);
  assert.equal(fits(rect(2, 12, 3, 2), land, [], c), true);
});

test('lot scenery follows standing buildings, clears on removal and reuses unchanged tiles', () => {
  const layer = new LotLayer();
  layer.setLots(packedLots(lot));
  layer.update();
  assert.equal(layer.group.children.length, 0, 'vacant land has no developed scenery');
  layer.setBuildings(packedBuildings(building()));
  layer.update();
  assert.equal(layer.group.children.length, 1);
  const mesh = layer.group.children[0].children[1];
  const geometry = mesh.geometry;
  layer.setBuildings(packedBuildings(building()));
  layer.update();
  assert.equal(layer.group.children[0].children[1], mesh, 'identical snapshots retain the mesh');
  assert.equal(mesh.geometry, geometry, 'identical snapshots retain geometry');
  layer.setLots(packedLots(lot));
  layer.update();
  assert.equal(layer.group.children[0].children[1], mesh, 'unchanged lots retain the mesh too');
  assert.equal(mesh.geometry, geometry, 'unchanged lots retain geometry too');
  let disposed = false;
  geometry.addEventListener('dispose', () => {
    disposed = true;
  });
  layer.setBuildings(packedBuildings());
  layer.update();
  assert.equal(layer.group.children.length, 0);
  assert.equal(disposed, true);
});

test('garden boundaries enclose all sides and meet at corners, leaving only the entrance open', () => {
  const mb = new MeshBuilder();
  boundary(mb, lot, [rect(11.25, -1, 5.5, 8)], new Clearance(), 'hedge');
  const p = mb.geometry().getAttribute('position');
  const sides = [[], [], [], []];
  for (let i = 0; i < p.count; i += 3) {
    if (![0, 1, 2].every((k) => Math.abs(p.getY(i + k) - 0.9) < 0.001)) continue;
    const xs = [0, 1, 2].map((k) => p.getX(i + k));
    const zs = [0, 1, 2].map((k) => p.getZ(i + k));
    const side =
      Math.max(...xs) < 1.1 ? 0 : Math.min(...xs) > 26.9 ? 1 : Math.min(...zs) > 38.9 ? 2 : 3;
    const values = side >= 2 ? xs : zs;
    sides[side].push([Math.min(...values), Math.max(...values)]);
  }
  assert.ok(
    sides.every((spans) => spans.length),
    'every side has a boundary, including the front',
  );
  for (const spans of sides.slice(0, 3)) {
    spans.sort((a, b) => a[0] - b[0]);
    let end = spans[0]?.[1];
    for (const [start, nextEnd] of spans.slice(1)) {
      assert.ok(start <= end + 0.01, 'no repeated gaps along a selected boundary');
      end = Math.max(end, nextEnd);
    }
    assert.ok(spans[0][0] < 0.7, 'boundary reaches its first corner');
    assert.ok(end > (spans === sides[2] ? 27.3 : 39.3), 'boundary reaches its last corner');
  }
  for (const [start, end] of sides[3])
    assert.ok(end <= 11.25 || start >= 16.75, 'front entrance remains open');
});

test('one obstruction cuts only its opening rather than removing the whole boundary', () => {
  const shapes = boundary(new MeshBuilder(), lot, [rect(12, 38, 4, 3)], new Clearance(), 'hedge');
  const covered = (p) => shapes.some((shape) => inside(p, shape));
  assert.ok(covered([5, 39.6]), 'rear boundary survives to the left of an obstruction');
  assert.ok(covered([22, 39.6]), 'rear boundary survives to the right of an obstruction');
  assert.equal(covered([14, 39.6]), false, 'the obstruction is not fenced through');
  for (const p of [
    [0.4, 2],
    [0.4, 38],
    [27.6, 2],
    [27.6, 38],
    [2, 0.4],
    [26, 0.4],
  ])
    assert.ok(covered(p), 'other sides reach their corners');
});

test('industrial boundaries are tall and neutral, never garden hedges', () => {
  let enclosed = 0;
  for (let i = 0; i < 16; i++) {
    const shifted = lot.map(([x, z]) => [x + i * 40, z]);
    const b = { ...building(5), ring: building(5).ring.map(([x, z]) => [x + i * 40, z]) };
    const g = buildLotGeometry(shifted, b, new Clearance(), false);
    const p = g.getAttribute('position');
    const colors = g.getAttribute('color');
    let high = 0;
    for (let k = 0; k < p.count; k++) {
      const x = p.getX(k) - i * 40;
      if (x > 1 && x < 27 && p.getZ(k) > 1 && p.getZ(k) < 39) continue;
      high = Math.max(high, p.getY(k));
      assert.ok(Math.abs(colors.getX(k) - colors.getY(k)) < 0.08, 'neutral, not green hedging');
    }
    if (high > 0) {
      enclosed++;
      assert.ok(high >= 1.7, 'an industrial-height fence or wall');
    }
  }
  assert.ok(enclosed > 0, 'most yards are enclosed');
});

test('shallow shop setbacks still fit entrance planters without reaching asphalt', () => {
  const roads = new Clearance();
  roads.setRoads(new Float32Array([-10, -4, 30, -4, 0]));
  const b = { ...building(3), ring: rect(0, 0.8, 14, 19.2) };
  const g = buildLotGeometry(rect(0, 0, 14, 20), b, roads, false);
  const p = g.getAttribute('position');
  assert.ok(p.count > 0, 'a shop occupying the lot still gets its two slim planters');
  for (let i = 0; i < p.count; i++) {
    assert.ok(p.getZ(i) > 0 && p.getZ(i) < 0.8);
    assert.ok(roads.roadGap(p.getX(i), p.getZ(i)) > 0);
  }
});

test('irregular and rotated lots never put props through walls or property edges', () => {
  const shapes = [
    [
      [0, 0],
      [28, 0],
      [22, 40],
      [3, 34],
    ],
    [
      [0, 0],
      [28, 0],
      [28, 40],
      [16, 40],
      [16, 25],
      [0, 25],
    ],
  ];
  for (const shape of shapes) {
    for (const angle of [0, 0.7, 1.6]) {
      const rotate = ([x, y]) => [
        x * Math.cos(angle) - y * Math.sin(angle),
        x * Math.sin(angle) + y * Math.cos(angle),
      ];
      const ring = shape.map(rotate);
      for (const zone of [1, 5]) {
        const b = { ...building(zone), ring: building().ring.map(rotate) };
        const p = buildLotGeometry(ring, b, new Clearance(), false).getAttribute('position');
        for (let i = 0; i < p.count; i++) {
          const point = [p.getX(i), p.getZ(i)];
          assert.ok(inside(point, ring), 'props stay inside irregular lots');
          assert.ok(!inside(point, b.ring), 'props stay outside building walls');
        }
      }
    }
  }
});

test('lots and buildings match by location, not packed snapshot order', () => {
  const layer = new LotLayer();
  const far = { ...building(5), ring: rect(307, 6, 14, 14) };
  layer.setLots(packedLots(rect(300, 0, 28, 40), lot));
  layer.setBuildings(packedBuildings(building(), far));
  layer.update();
  assert.equal(layer.group.children.length, 2);
  layer.setLots(packedLots(lot));
  layer.update();
  assert.equal(layer.group.children.length, 1, 'removed lots leave no stale scenery');
});

test('lot tiles drop their vertices once drawn and rebuild them when the GPU loses them', () => {
  const layer = new LotLayer();
  layer.setLots(packedLots(lot));
  layer.setBuildings(packedBuildings(building()));
  layer.update();
  const meshes = () => layer.group.children.flatMap((tile) => tile.children);
  const arrays = () =>
    meshes().flatMap((m) => Object.values(m.geometry.attributes).map((a) => (a.data ?? a).array));
  // What the renderer does when it first draws a mesh: hands each vertex array to the GPU.
  for (const m of meshes())
    for (const a of Object.values(m.geometry.attributes)) (a.data ?? a).onUploadCallback();
  assert.ok(arrays().every((a) => a === null));
  const before = meshes();
  layer.restore();
  assert.equal(meshes().length, before.length);
  assert.ok(meshes().every((m, i) => m !== before[i] && m.geometry.getAttribute('position').count));
  assert.ok(
    arrays().every((a) => a !== null),
    'arrays to upload again',
  );
});
