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
// A 25 m hall front has three loading doors, at 3–7, 10–14 and 17–21 m along it.
const fixture = (x = 0, zone = 5) => ({
  lot: rect(x, 0, 48, 64),
  b: { zone, height: 8, roof: 0, front: 0, ring: rect(x + 11.5, 12, 25, 26) },
});
const draw = ({ lot, b }, roads = new Clearance()) => {
  const props = new MeshBuilder();
  const surface = new MeshBuilder();
  lotGeometry(props, lot, b, roads, surface);
  return { props: props.geometry(), surface: surface.geometry() };
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
const yardOf = ({ props, surface }) => {
  const containers = [0x9b4a3c, 0x3f6d8c, 0x6f7f4a, 0xb08a3e].some((c) => colored(props, c).length);
  const tanks = colored(props, 0xc9ccc6).length > 0;
  const pallets = colored(props, 0xa58a65).length > 0;
  const bays = colored(surface, 0x6f7470).length / 6;
  return containers
    ? 'containers'
    : tanks
      ? 'tanks'
      : pallets
        ? 'pallets'
        : bays > 1
          ? 'parking'
          : 'open';
};

test('factory yards choose one main use instead of repeating every prop', () => {
  const uses = new Set();
  for (let i = 0; i < 48; i++) {
    const g = draw(fixture(i * 60, 5 + (i % 2)));
    const kinds = [
      [0x9b4a3c, 0x3f6d8c, 0x6f7f4a, 0xb08a3e].some((c) => colored(g.props, c).length),
      colored(g.props, 0xc9ccc6).length > 0,
      colored(g.props, 0xa58a65).length > 0,
    ];
    assert.ok(kinds.filter(Boolean).length <= 1, 'containers, tanks and pallets do not mix');
    uses.add(yardOf(g));
  }
  assert.deepEqual(uses, new Set(['containers', 'tanks', 'pallets', 'parking']));
});

test('industrial ground and boundaries vary between sites', () => {
  const grounds = new Set();
  const boundaries = new Set();
  for (let i = 0; i < 48; i++) {
    const g = draw(fixture(i * 60));
    const c = g.surface.getAttribute('color');
    grounds.add(new THREE.Color(c.getX(0), c.getY(0), c.getZ(0)).getHex());
    const wall = colored(g.props, 0xa39d92).length > 0;
    const fence = colored(g.props, 0x737d80).length > 0;
    boundaries.add(wall ? 'wall' : fence ? 'metal' : 'none');
  }
  assert.equal(grounds.size, 3, 'asphalt, concrete and gravel yards');
  assert.deepEqual(boundaries, new Set(['metal', 'wall', 'none']));
});

test('aprons connect the frontage to loading doors and keep the gate open', () => {
  let courts = 0;
  for (let i = 0; i < 32; i++) {
    const site = fixture(i * 60);
    const x0 = i * 60 + 11.5;
    const { props, surface } = draw(site);
    const lines = colored(surface, 0xcfb35a);
    const apron = [0xaaa99e, 0x858b87]
      .flatMap((c) => colored(surface, c))
      .filter((p) => p[2] < 12 && p[2] > 0);
    assert.ok(apron.length, 'every hall has an apron');
    assert.ok(Math.min(...apron.map((p) => p[2])) < 0.4, 'the apron reaches the frontage');
    const xs = apron.map((p) => p[0] - x0);
    if (lines.length) {
      courts++;
      assert.ok(Math.min(...xs) <= 3 && Math.max(...xs) >= 21, 'a court spans all three doors');
    } else assert.ok(Math.min(...xs) <= 10 && Math.max(...xs) >= 14, 'a drive meets a door');
    const pos = props.getAttribute('position');
    const gate = (Math.min(...xs) + Math.max(...xs)) / 2 + x0;
    for (let k = 0; k < pos.count; k += 3) {
      const triangle = [0, 1, 2].map((j) => [pos.getX(k + j), pos.getZ(k + j)]);
      assert.ok(!inside([gate, 0.4], triangle), 'no fence or wall crosses the gate');
    }
  }
  assert.ok(courts > 0 && courts < 32, 'some truck courts, some single drives');
});

test('yard choices survive rebuilds and props clear walls, streets and lot edges', () => {
  const roads = new Clearance();
  roads.setRoads(new Float32Array([-20, -4, 3000, -4, 0]));
  for (let i = 0; i < 16; i++) {
    const site = fixture(i * 60);
    const first = draw(site, roads);
    const again = draw(structuredClone(site), roads);
    assert.deepEqual(
      [...first.props.getAttribute('position').array, ...first.props.getAttribute('color').array],
      [...again.props.getAttribute('position').array, ...again.props.getAttribute('color').array],
    );
    const p = first.props.getAttribute('position');
    for (let k = 0; k < p.count; k++) {
      const point = [p.getX(k), p.getZ(k)];
      assert.ok(inside(point, site.lot), 'props stay on the property');
      assert.ok(!inside(point, site.b.ring), 'props stay outside the hall');
      assert.ok(roads.roadGap(...point) > 0, 'props stay off the road');
    }
  }
});
