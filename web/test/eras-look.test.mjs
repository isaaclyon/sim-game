import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { archetype } from '../src/building-kit/kit.ts';
import { PRESENT_DAY, VILLAGE } from '../src/building-kit/style.ts';
import { DIRT_ROADS, ROADS, SIDEWALK, VERGE } from '../src/palette.ts';
import { Era } from '../src/protocol.ts';
import { RoadSurface } from '../src/road-surface.ts';

const ring = (x) => [
  [x, 0],
  [x + 12, 0],
  [x + 12, 11],
  [x, 11],
];
const building = (zone, era, x) => ({ zone, height: 6, roof: 0, front: 0, era, ring: ring(x) });
const VILLAGE_DESIGNS = {
  1: ['cottage', 'thatched', 'gable', 'saltbox', 'jerkinhead', 'porch'],
  3: ['hip', 'false-front', 'gable-front'],
  5: ['classic', 'brick'],
};

test('village buildings draw only village designs, and keep them beside town ones', () => {
  for (const [zone, names] of Object.entries(VILLAGE_DESIGNS)) {
    const seen = new Set();
    for (let i = 0; i < 200; i++) {
      const name = archetype(building(Number(zone), Era.Village, i * 30));
      assert.ok(names.includes(name), `zone ${zone} village draws ${name}`);
      seen.add(name);
    }
    assert.deepEqual([...seen].sort(), [...names].sort(), `zone ${zone} uses every design`);
  }
  // The same lot built in the town era can draw a design the village never uses.
  const town = Array.from({ length: 200 }, (_, i) => archetype(building(1, Era.Town, i * 30)));
  assert.ok(town.includes('modern') && town.includes('mansard'));
});

test('a village building takes the village palette', () => {
  const colours = (era) => {
    const b = building(1, era, 0);
    const data = new Float32Array([1, 6, 0, 0, era, 4, ...b.ring.flat()]);
    const c = buildBuildingGeometry(data).getAttribute('color');
    const out = new Set();
    for (let i = 0; i < c.count; i++)
      out.add(new THREE.Color(c.getX(i), c.getY(i), c.getZ(i)).getHex());
    return out;
  };
  const roofs = (style) => style.resLow.roofs.map((h) => new THREE.Color(h).getHex());
  const has = (set, list) => list.some((h) => set.has(h));
  assert.ok(
    has(colours(Era.Village), roofs(VILLAGE)) && !has(colours(Era.Village), roofs(PRESENT_DAY)),
  );
  assert.ok(has(colours(Era.Town), roofs(PRESENT_DAY)) && !has(colours(Era.Town), roofs(VILLAGE)));
});

test('roads are dirt without paint in a village and asphalt once the town arrives', () => {
  const surface = new RoadSurface();
  const [sidewalks, roads, markings] = surface.group.children;
  surface.set(new Float32Array([0, 0, 100, 0, 0, 0, 50, 100, 50, 1]));
  const first = () => {
    const c = roads.geometry.getAttribute('color');
    return new THREE.Color(c.getX(0), c.getY(0), c.getZ(0)).getHex();
  };
  const linear = (hex) => new THREE.Color(hex).getHex();
  assert.ok(ROADS.map(linear).includes(first()), 'a town until the sim says otherwise');
  surface.setEra(Era.Village);
  assert.ok(DIRT_ROADS.map(linear).includes(first()));
  assert.equal(markings.visible, false, 'dirt tracks have no lane paint');
  assert.equal(sidewalks.material.color.getHex(), linear(VERGE));
  surface.setEra(Era.Town);
  assert.ok(ROADS.map(linear).includes(first()));
  assert.equal(markings.visible, true);
  assert.equal(sidewalks.material.color.getHex(), linear(SIDEWALK));
});
