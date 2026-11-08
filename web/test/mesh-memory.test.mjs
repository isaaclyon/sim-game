import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { MeshBuilder } from '../src/building-kit/mesh.ts';
import { lotGeometry } from '../src/lot-scenery/geometry.ts';
import { Clearance } from '../src/scenery/clearance.ts';

const rect = (x, y, w, d) => [
  [x, y],
  [x + w, y],
  [x + w, y + d],
  [x, y + d],
];
const record = (zone, ring) => [zone, 15, 0, 0, ring.length, ...ring.flat()];

/** Bytes the geometry holds per vertex, counting each shared interleaved buffer once. */
function bytesPerVertex(g) {
  const arrays = new Set(Object.values(g.attributes).map((a) => (a.data ?? a).array));
  let bytes = 0;
  for (const array of arrays) bytes += array.byteLength;
  return bytes / g.getAttribute('position').count;
}

test('lit buildings keep at most 32 bytes per vertex in memory', () => {
  // Position 12, normal 4, colour 8, night glow 4, light schedule 4.
  const data = [];
  for (let zone = 1; zone <= 6; zone++) data.push(...record(zone, rect(zone * 40, 0, 18, 14)));
  const g = buildBuildingGeometry(new Float32Array(data));
  assert.ok(g.getAttribute('nightEmission'), 'some windows glow');
  assert.ok(g.getAttribute('lightSchedule'), 'some buildings keep evening hours');
  assert.ok(bytesPerVertex(g) <= 32, `${bytesPerVertex(g)} bytes per vertex`);
});

test('unlit outdoor scenery keeps 24 bytes per vertex and no night attributes', () => {
  const mb = new MeshBuilder();
  for (let zone = 1; zone <= 6; zone++) {
    const lot = rect(zone * 40, 0, 28, 40);
    const building = { zone, height: 6, roof: 0, front: 0, ring: rect(zone * 40 + 7, 6, 14, 14) };
    lotGeometry(mb, lot, building, new Clearance());
  }
  const g = mb.geometry();
  assert.equal(g.getAttribute('nightEmission'), undefined);
  assert.equal(g.getAttribute('lightSchedule'), undefined);
  assert.equal(bytesPerVertex(g), 24);
});

test('compact attributes keep colours, normals and glow within a step of the source', () => {
  const mb = new MeshBuilder();
  const wall = new THREE.Color(0x56794d);
  mb.glow.set(0xffcf83);
  mb.lightSchedule = [17.3, 29.6];
  mb.quad([0, 0, 0], [4, 0, 0], [4, 3, 0], [0, 3, 0], wall, [0, 0, 1]);
  const g = mb.geometry();
  const [n, c, e, s] = ['normal', 'color', 'nightEmission', 'lightSchedule'].map((a) =>
    g.getAttribute(a),
  );
  const glow = new THREE.Color(0xffcf83);
  for (let i = 0; i < 6; i++) {
    assert.deepEqual([n.getX(i), n.getY(i), n.getZ(i)], [0, 0, 1]);
    // Colours round-trip to their palette hex; glow keeps byte precision.
    assert.equal(new THREE.Color(c.getX(i), c.getY(i), c.getZ(i)).getHex(), 0x56794d);
    for (const [got, want] of [
      [e.getX(i), glow.r],
      [e.getY(i), glow.g],
      [e.getZ(i), glow.b],
    ])
      assert.ok(Math.abs(got - want) <= 0.5 / 255, `${got} vs ${want}`);
    // Within one clock minute of the scheduled hours.
    assert.ok(Math.abs(s.getX(i) - 17.3) < 1 / 60 && Math.abs(s.getY(i) - 29.6) < 1 / 60);
  }
});
