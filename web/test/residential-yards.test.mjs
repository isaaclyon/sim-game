import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { building as drawBuilding } from '../src/building-kit/kit.ts';
import { MeshBuilder } from '../src/building-kit/mesh.ts';
import { inside } from '../src/building-kit/polygon.ts';
import { PRESENT_DAY } from '../src/building-kit/style.ts';
import { boundary } from '../src/lot-scenery/boundary.ts';
import { lotGeometry } from '../src/lot-scenery/geometry.ts';
import { Clearance } from '../src/scenery/clearance.ts';

const rect = (x, y, w, d) => [
  [x, y],
  [x + w, y],
  [x + w, y + d],
  [x, y + d],
];
const fixture = (x = 0, setback = 10) => ({
  lot: rect(x, 0, 28, 48),
  b: { zone: 1, height: 6, roof: 0, front: 0, ring: rect(x + 7, setback, 14, 14) },
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

test('house paths are narrow, align with the actual door and reach the street-facing lot edge', () => {
  for (let i = 0; i < 24; i++) {
    const site = fixture(i * 40);
    const house = new MeshBuilder();
    drawBuilding(house, site.b, PRESENT_DAY);
    const door = colored(house.geometry(), PRESENT_DAY.door);
    const doorX = (Math.min(...door.map((p) => p[0])) + Math.max(...door.map((p) => p[0]))) / 2;
    const paths = colored(draw(site).surface, 0xc8bda8).filter((p) => p[2] < 10);
    assert.ok(paths.length, 'a visible entrance path');
    const xs = paths.map((p) => p[0]);
    assert.ok(Math.max(...xs) - Math.min(...xs) <= 2, 'a footpath, not a broad paved front yard');
    assert.ok(doorX > Math.min(...xs) && doorX < Math.max(...xs), 'the path meets the front door');
    assert.ok(Math.min(...paths.map((p) => p[2])) < 0.4, 'the path reaches the frontage');
    assert.ok(Math.max(...paths.map((p) => p[2])) > 9.8, 'the path reaches the house');
  }
});

test('residential boundaries mix continuous hedges, pointed white pickets and open lots', () => {
  const site = fixture();
  const pickets = new MeshBuilder();
  const reserved = boundary(pickets, site.lot, [], new Clearance(), 'picket');
  assert.ok(reserved.length >= 4, 'all property sides are reserved together');
  const p = pickets.geometry().getAttribute('position');
  const white = colored(pickets.geometry(), 0xeee9dc);
  assert.ok(white.length > 0, 'warm-white fencing');
  assert.ok(
    Array.from({ length: p.count }, (_, i) => p.getY(i)).some((y) => Math.abs(y - 1) < 0.001),
    'pointed picket tips',
  );
  const open = new MeshBuilder();
  assert.deepEqual(boundary(open, site.lot, [], new Clearance(), 'none'), []);
  assert.equal(open.vertexCount, 0, 'open lots have no invisible boundary reservation');
  const styles = new Set();
  for (let i = 0; i < 48; i++) {
    const { props } = draw(fixture(i * 40));
    const pos = props.getAttribute('position');
    const col = props.getAttribute('color');
    let style = 'none';
    for (let k = 0; k < pos.count; k++) {
      if (pos.getX(k) - i * 40 > 1) continue;
      style = near(col, k, new THREE.Color(0xeee9dc)) ? 'picket' : 'hedge';
      break;
    }
    styles.add(style);
  }
  assert.deepEqual(styles, new Set(['hedge', 'picket', 'none']));
});

test('front gardens and usable parking spaces vary between houses without paving every yard', () => {
  let driveways = 0;
  let frontGardens = 0;
  let lawns = 0;
  for (let i = 0; i < 48; i++) {
    const site = fixture(i * 40);
    const { props, surface } = draw(site);
    const parking = colored(surface, 0x858b87);
    const flowers = [0xc88791, 0xe0c077, 0xb4a0c7]
      .flatMap((c) => colored(props, c))
      .filter((p) => p[2] < 10);
    if (parking.length) {
      driveways++;
      const xs = parking.map((p) => p[0]);
      const zs = parking.map((p) => p[2]);
      assert.ok(Math.max(...xs) - Math.min(...xs) >= 2.6, 'a car-width space');
      assert.ok(Math.max(...zs) - Math.min(...zs) >= 5, 'enough length to park');
      assert.ok(Math.min(...zs) < 0.4, 'the driveway connects to the frontage');
      const end = Math.max(...zs);
      assert.ok(
        colored(props, 0xaaa99e).some((p) => p[2] > end - 0.6 && p[2] < end),
        'a wheel stop marks parking, not a nonexistent garage',
      );
    } else if (flowers.length) frontGardens++;
    else lawns++;
  }
  assert.ok(
    driveways > 0 && frontGardens > 0 && lawns > 0,
    'parking, gardens and quieter lawns all occur',
  );
});

test('backyards choose one treatment instead of repeating every prop on every lot', () => {
  const treatments = new Set();
  for (let i = 0; i < 48; i++) {
    const { props, surface } = draw(fixture(i * 40));
    const p = props.getAttribute('position');
    const rear = Array.from({ length: p.count }, (_, k) => [
      p.getX(k) - i * 40,
      p.getY(k),
      p.getZ(k),
    ]).filter(([x, , z]) => x > 2 && x < 26 && z > 24.5 && z < 46);
    const beds = colored(props, 0x76644c).some((p) => p[2] > 24.5);
    const patio = colored(surface, 0xc8bda8).some((p) => p[2] > 24.5);
    const tree = rear.some((p) => p[1] > 3);
    assert.ok(Number(beds) + Number(patio) + Number(tree) <= 1, 'one main backyard use');
    treatments.add(beds ? 'vegetables' : patio ? 'patio' : tree ? 'tree' : 'lawn');
  }
  assert.deepEqual(treatments, new Set(['lawn', 'vegetables', 'patio', 'tree']));
});

test('front gates stay open for both footpaths and driveways', () => {
  for (let i = 0; i < 24; i++) {
    const { props, surface } = draw(fixture(i * 40));
    const pos = props.getAttribute('position');
    for (const color of [0xc8bda8, 0x858b87]) {
      const tips = colored(surface, color).filter((p) => p[2] < 0.4);
      if (!tips.length) continue;
      const x = (Math.min(...tips.map((p) => p[0])) + Math.max(...tips.map((p) => p[0]))) / 2;
      for (let k = 0; k < pos.count; k += 3) {
        const triangle = [0, 1, 2].map((j) => [pos.getX(k + j), pos.getZ(k + j)]);
        for (const offset of [-0.4, 0, 0.4])
          assert.ok(
            !inside([x + offset, 0.4], triangle),
            'no fence or planting crosses the entrance',
          );
      }
    }
  }
});

test('tight front yards omit parking while deep setbacks still connect to the frontage', () => {
  for (let i = 0; i < 16; i++) {
    const x = i * 40;
    const tight = { lot: rect(x, 0, 16, 32), b: { ...fixture().b, ring: rect(x + 2, 4, 12, 14) } };
    const geometry = draw(tight);
    assert.equal(colored(geometry.surface, 0x858b87).length, 0, 'no car-sized parking space fits');
    const path = colored(geometry.surface, 0xc8bda8).filter((p) => p[2] < 4);
    assert.ok(path.length, 'a small yard still has an entrance');
  }
  const deep = { lot: rect(0, 0, 28, 110), b: { ...fixture().b, ring: rect(7, 80, 14, 14) } };
  const path = colored(draw(deep).surface, 0xc8bda8).filter((p) => p[2] < 80);
  assert.ok(
    path.some((p) => p[2] < 0.4),
    'no fixed-length path leaves a deep setback disconnected',
  );
});

test('yard choices survive rebuilds and props clear walls, streets and irregular lot edges', () => {
  const roads = new Clearance();
  roads.setRoads(new Float32Array([-20, -4, 100, -4, 0]));
  const sites = [
    fixture(),
    fixture(40, 1),
    {
      lot: [
        [0, 0],
        [28, 0],
        [25, 48],
        [16, 48],
        [16, 34],
        [2, 34],
      ],
      b: fixture().b,
    },
  ];
  for (const site of sites) {
    const first = draw(site, roads);
    const again = draw(structuredClone(site), roads);
    assert.deepEqual(
      [...first.props.getAttribute('position').array, ...first.props.getAttribute('color').array],
      [...again.props.getAttribute('position').array, ...again.props.getAttribute('color').array],
    );
    assert.deepEqual(
      [
        ...first.surface.getAttribute('position').array,
        ...first.surface.getAttribute('color').array,
      ],
      [
        ...again.surface.getAttribute('position').array,
        ...again.surface.getAttribute('color').array,
      ],
    );
    const p = first.props.getAttribute('position');
    assert.ok(p.count < 18000, 'fences remain modest batched geometry');
    for (let k = 0; k < p.count; k++) {
      const point = [p.getX(k), p.getZ(k)];
      assert.ok(inside(point, site.lot), 'props stay on the property');
      assert.ok(!inside(point, site.b.ring), 'props stay outside the house');
      assert.ok(roads.roadGap(...point) > 0, 'props stay off the road');
    }
  }
});
