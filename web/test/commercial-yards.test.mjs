import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { MeshBuilder } from '../src/building-kit/mesh.ts';
import { inside } from '../src/building-kit/polygon.ts';
import { lotGeometry } from '../src/lot-scenery/geometry.ts';
import { Clearance } from '../src/scenery/clearance.ts';

const rect = (x, y, w, d) => [
  [x, y],
  [x + w, y],
  [x + w, y + d],
  [x, y + d],
];
// A typical small shop: 1.5 m side gaps, about 2 m back from the frontage, a deep rear lot.
// Its door is 30% along the front, at x + 6.
const fixture = (x = 0) => ({
  lot: rect(x, 0, 18, 56),
  b: { zone: 3, height: 4, roof: 0, front: 0, ring: rect(x + 1.5, 2.2, 15, 12) },
});
const draw = ({ lot, b }, roads = new Clearance()) => {
  const props = new MeshBuilder();
  const surface = new MeshBuilder();
  lotGeometry(props, lot, b, roads, surface);
  // Shared corners expanded, so checks can read triangles as runs of three vertices.
  return { props: props.geometry().toNonIndexed(), surface: surface.geometry().toNonIndexed() };
};
const near = (colors, i, color) =>
  Math.abs(colors.getX(i) - color.r) +
    Math.abs(colors.getY(i) - color.g) +
    Math.abs(colors.getZ(i) - color.b) <
  0.001;
const colored = (geometry, hex) => {
  const p = geometry.getAttribute('position');
  const c = geometry.getAttribute('color');
  const color = new THREE.Color(hex);
  return Array.from({ length: p.count }, (_, i) => i)
    .filter((i) => near(c, i, color))
    .map((i) => [p.getX(i), p.getY(i), p.getZ(i)]);
};
/** The colour drawn last, so on top, at a ground point; null where nothing covers it. */
const topColor = (surface, x, z) => {
  const p = surface.getAttribute('position');
  const c = surface.getAttribute('color');
  let top = null;
  for (let k = 0; k < p.count; k += 3) {
    const triangle = [0, 1, 2].map((j) => [p.getX(k + j), p.getZ(k + j)]);
    if (inside([x, z], triangle)) top = new THREE.Color(c.getX(k), c.getY(k), c.getZ(k)).getHex();
  }
  return top;
};
const lawns = new Set([0x8fa66c, 0x98ad75, 0x849c64, 0x9aab79]);
const pavings = new Set([0xc8bda8, 0xb3b0a6, 0xa9826e]);
const has = (geometry, hexes, test = () => true) =>
  hexes.some((h) => colored(geometry, h).some(test));
const frontOf = ({ props }) => {
  const front = (p) => p[2] < 2.2;
  if (has(props, [0xe4ddcc], front)) return 'cafe';
  if (has(props, [0x4a4f52], front)) return 'bollards';
  if (has(props, [0x5f676b], front)) return 'cycles';
  return has(props, [0x56794d], front) ? 'planters' : 'none';
};
const rearOf = ({ props }) => {
  const rear = (p) => p[2] > 14.2;
  if (has(props, [0xd9cfb8, 0x9a4f4f, 0x4f6f86], rear)) return 'terrace';
  if (has(props, [0x3f5a46, 0x6b6f72], rear)) return 'service';
  if (has(props, [0x6b9054, 0x7e9b5e, 0x527c51], rear)) return 'garden';
  return 'lawn';
};

test('shop frontages vary between planters, cafe tables, cycle hoops and bollards', () => {
  const kinds = new Set();
  for (let i = 0; i < 48; i++) kinds.add(frontOf(draw(fixture(i * 30))));
  assert.deepEqual(kinds, new Set(['planters', 'cafe', 'cycles', 'bollards']));
});

test('shop rear spaces choose one use: garden, terrace, service corner or open lawn', () => {
  const kinds = new Set();
  for (let i = 0; i < 48; i++) {
    const g = draw(fixture(i * 30));
    const rear = (p) => p[2] > 14.2;
    const uses = [
      has(g.props, [0xd9cfb8, 0x9a4f4f, 0x4f6f86], rear),
      has(g.props, [0x3f5a46, 0x6b6f72], rear),
      has(g.props, [0x6b9054, 0x7e9b5e, 0x527c51], rear),
    ];
    assert.ok(uses.filter(Boolean).length <= 1, 'one main rear use');
    kinds.add(rearOf(g));
  }
  assert.deepEqual(kinds, new Set(['garden', 'terrace', 'service', 'lawn']));
});

test('forecourt paving varies and the shop door stays clear', () => {
  const pavings = new Set();
  for (let i = 0; i < 32; i++) {
    const x = i * 30;
    const { props, surface } = draw(fixture(x));
    for (const hex of [0xc8bda8, 0xb3b0a6, 0xa9826e])
      if (colored(surface, hex).some((p) => p[2] < 2.2)) pavings.add(hex);
    const pos = props.getAttribute('position');
    for (let k = 0; k < pos.count; k += 3) {
      const triangle = [0, 1, 2].map((j) => [pos.getX(k + j), pos.getZ(k + j)]);
      for (const z of [0.4, 1.2, 2])
        for (const dx of [-0.6, 0, 0.6])
          assert.ok(!inside([x + 6 + dx, z], triangle), 'nothing blocks the way to the door');
    }
  }
  assert.equal(pavings.size, 3, 'sand, grey slab and brick forecourts');
});

test('shops set back from the street have one asphalt car park in front, never grass', () => {
  for (let i = 0; i < 16; i++) {
    const x = i * 30;
    // Pushed 10 m back, as the sim does when the nearest spot does not fit.
    const site = { ...fixture(x), b: { ...fixture(x).b, ring: rect(x + 1.5, 10, 15, 12) } };
    const { props, surface } = draw(site);
    const bays = colored(surface, 0x858b87).filter((p) => p[2] < 10);
    assert.ok(bays.length, 'front parking');
    assert.ok(Math.min(...bays.map((p) => p[2])) < 0.4, 'bays connect to the street');
    const lines = colored(surface, 0xd4cdb7).filter((p) => p[2] < 10);
    assert.ok(lines.length >= 4 * 6, 'painted bay lines');
    // The door is at x + 6. Sample the whole frontage between the shop and the street.
    for (let dx = 0.5; dx < 15; dx += 0.5)
      for (const z of [0.6, 4, 9.2]) {
        const top = topColor(surface, x + 1.5 + dx, z);
        assert.ok(!lawns.has(top), `no grass in front of the shop at ${dx}, ${z}`);
      }
    for (const z of [0.6, 4, 9.2])
      assert.equal(topColor(surface, x + 6, z), 0x858b87, 'one surface up to the door');
    const doorLines = lines.filter((p) => Math.abs(p[0] - (x + 6)) < 1);
    assert.equal(doorLines.length, 0, 'no bay marked in front of the door');
    assert.ok(frontOf({ props }) !== 'cafe', 'parking replaces frontage tables');
  }
});

test('storage yards behind small shops have a shed and stacked stock', () => {
  let yards = 0;
  for (let i = 0; i < 48; i++) {
    const { props } = draw(fixture(i * 30));
    const shed = colored(props, 0x8c9396);
    if (!shed.length) continue;
    yards++;
    assert.ok(
      shed.every((p) => p[2] > 14.2),
      'the shed stands behind the shop',
    );
    assert.ok(colored(props, 0xb4996e).length > 0, 'stock on pallets');
    assert.ok(has(props, [0x3f5a46, 0x6b6f72]), 'bins');
  }
  assert.ok(yards >= 8, 'storage is a common rear use');
});

test('no small shop has bare grass in front, on tapered lots or at any setback', () => {
  const roads = new Clearance();
  roads.setRoads(new Float32Array([-20, -4, 2000, -4, 0]));
  for (let i = 0; i < 24; i++) {
    const x = i * 30;
    for (const setback of [2.2, 5, 8, 16, 24]) {
      // The lot narrows by 0.6 m on each side towards the street, as cut lots often do.
      const lot = [
        [x + 0.6, 0],
        [x + 17.4, 0],
        [x + 18, 56],
        [x, 56],
      ];
      const b = { ...fixture(x).b, ring: rect(x + 1.2, setback, 15.6, 12) };
      const { surface } = draw({ lot, b }, roads);
      for (let dx = 0.3; dx < 15.6; dx += 0.6)
        for (let z = 0.3; z < setback; z += 0.7) {
          const top = topColor(surface, x + 1.2 + dx, z);
          assert.ok(!lawns.has(top), `grass in front of a shop ${setback} m back at ${dx}, ${z}`);
        }
    }
  }
});

test('small shop forecourts are paved on concave lots too', () => {
  for (let i = 0; i < 16; i++) {
    const x = i * 30;
    // A notch at the back makes the lot concave; the front is still straight.
    const lot = [
      [x, 0],
      [x + 18, 0],
      [x + 18, 56],
      [x + 10, 56],
      [x + 10, 40],
      [x + 8, 40],
      [x + 8, 56],
      [x, 56],
    ];
    const { surface } = draw({ lot, b: fixture(x).b });
    for (const dx of [1, 6, 14]) assert.ok(pavings.has(topColor(surface, x + 1.5 + dx, 1)));
  }
});

test('dense blocks stay paved and low, and choices survive rebuilds clear of roads', () => {
  const roads = new Clearance();
  roads.setRoads(new Float32Array([-20, -4, 2000, -4, 0]));
  for (let i = 0; i < 24; i++) {
    const site = fixture(i * 30);
    for (const zone of [3, 4]) {
      const b = { ...site.b, zone };
      const first = draw({ ...site, b }, roads);
      const again = draw(structuredClone({ ...site, b }), roads);
      assert.deepEqual(
        [...first.props.getAttribute('position').array, ...first.props.getAttribute('color').array],
        [...again.props.getAttribute('position').array, ...again.props.getAttribute('color').array],
      );
      const p = first.props.getAttribute('position');
      for (let k = 0; k < p.count; k++) {
        const point = [p.getX(k), p.getZ(k)];
        assert.ok(inside(point, site.lot), 'props stay on the property');
        assert.ok(!inside(point, b.ring), 'props stay outside the shop');
        assert.ok(roads.roadGap(...point) > 0, 'props stay off the road');
        if (zone === 4) assert.ok(p.getY(k) < 2, 'no trees or parasols on dense lots');
      }
    }
  }
});
