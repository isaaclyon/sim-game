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

/** Bytes per triangle, vertices and index together. */
const bytesPerTriangle = (g) =>
  (bytesPerVertex(g) * g.getAttribute('position').count + g.index.array.byteLength) /
  (g.index.count / 3);

test('lit buildings keep at most 28 bytes per vertex and 72 per triangle in memory', () => {
  // Position 12, normal 4, colour 8, night glow 4; light hours only in the compact format.
  const data = [];
  for (let zone = 1; zone <= 6; zone++) data.push(...record(zone, rect(zone * 40, 0, 18, 14)));
  const g = buildBuildingGeometry(new Float32Array(data));
  assert.ok(g.getAttribute('nightEmission'), 'some windows glow');
  assert.equal(g.getAttribute('lightOn'), undefined);
  assert.ok(bytesPerVertex(g) <= 28, `${bytesPerVertex(g)} bytes per vertex`);
  assert.ok(bytesPerTriangle(g) <= 72, `${bytesPerTriangle(g)} bytes per triangle`);
});

test('unlit outdoor scenery keeps 24 bytes per vertex, 64 per triangle, no night attributes', () => {
  const mb = new MeshBuilder();
  for (let zone = 1; zone <= 6; zone++) {
    const lot = rect(zone * 40, 0, 28, 40);
    const building = { zone, height: 6, roof: 0, front: 0, ring: rect(zone * 40 + 7, 6, 14, 14) };
    lotGeometry(mb, lot, building, new Clearance());
  }
  const g = mb.geometry();
  assert.equal(g.getAttribute('nightEmission'), undefined);
  assert.equal(bytesPerVertex(g), 24);
  // A triangle soup takes 72.
  assert.ok(bytesPerTriangle(g) <= 64, `${bytesPerTriangle(g)} bytes per triangle`);
});

test('stored colours, normals and glow stay within a step of the source', () => {
  const mb = new MeshBuilder();
  const wall = new THREE.Color(0x56794d);
  mb.glow.set(0xffcf83);
  mb.quad([0, 0, 0], [4, 0, 0], [4, 3, 0], [0, 3, 0], wall, [0, 0, 1]);
  const g = mb.geometry();
  const [n, c, e] = ['normal', 'color', 'nightEmission'].map((a) => g.getAttribute(a));
  const glow = new THREE.Color(0xffcf83);
  assert.deepEqual([n.count, g.index.count], [4, 6], 'two triangles share four corners');
  for (let i = 0; i < n.count; i++) {
    assert.deepEqual([n.getX(i), n.getY(i), n.getZ(i)], [0, 0, 1]);
    // Colours round-trip to their palette hex; glow keeps byte precision.
    assert.equal(new THREE.Color(c.getX(i), c.getY(i), c.getZ(i)).getHex(), 0x56794d);
    for (const [got, want] of [
      [e.getX(i), glow.r],
      [e.getY(i), glow.g],
      [e.getZ(i), glow.b],
    ])
      assert.ok(Math.abs(got - want) <= 0.5 / 255, `${got} vs ${want}`);
  }
});

test('compact meshes keep palette colours exact, and glow and light hours within a step', () => {
  const mb = new MeshBuilder({ compact: true });
  mb.glow.set(0xffcf83);
  mb.lightSchedule = [17.3, 29.6];
  mb.quad([0, 0, 0], [4, 0, 0], [4, 3, 0], [0, 3, 0], new THREE.Color(0x56794d), [0, 0, 1]);
  const g = mb.geometry();
  const [c, e, on, off] = ['color', 'nightEmission', 'lightOn', 'lightOff'].map((a) =>
    g.getAttribute(a),
  );
  const glow = new THREE.Color(0xffcf83);
  for (let i = 0; i < c.count; i++) {
    // Bytes on the palette's own sRGB scale: shaders decode them back to linear.
    const srgb = new THREE.Color().setRGB(c.getX(i), c.getY(i), c.getZ(i), THREE.SRGBColorSpace);
    assert.equal(srgb.getHex(), 0x56794d);
    for (const [got, want] of [
      [e.getX(i), glow.r],
      [e.getY(i), glow.g],
      [e.getZ(i), glow.b],
    ])
      assert.ok(Math.abs(got - want) <= 0.5 / 255, `${got} vs ${want}`);
    // Bytes in eighths of an hour, so within four clock minutes.
    const hours = (v) => v * (255 / 8);
    assert.ok(Math.abs(hours(on.getX(i)) - 17.3) < 1 / 15, `${hours(on.getX(i))} on`);
    assert.ok(Math.abs(hours(off.getX(i)) - 29.6) < 1 / 15, `${hours(off.getX(i))} off`);
  }
  assert.equal(new MeshBuilder({ compact: true }).geometry().getAttribute('lightOn'), undefined);
});

/** Each triangle's corner positions and colours in draw order, however corners are stored. */
const triangles = (g) => {
  const flat = g.toNonIndexed();
  const color = flat.getAttribute('color');
  const linear = Array.from({ length: color.count }, (_, i) =>
    new THREE.Color().setRGB(color.getX(i), color.getY(i), color.getZ(i), srgb(color)).getHex(),
  );
  return [Array.from(flat.getAttribute('position').array), linear];
};
/** Compact meshes store sRGB bytes; others linear 16-bit values. */
const srgb = (color) =>
  (color.data ?? color).array instanceof Uint8Array
    ? THREE.SRGBColorSpace
    : THREE.LinearSRGBColorSpace;

test('compact meshes leave out normals and reuse corners of the same paint', () => {
  const mb = new MeshBuilder({ compact: true });
  mb.box([0, 0], 0, 4, 3, 2, 0.3, new THREE.Color(0x56794d));
  const g = mb.geometry();
  assert.equal(g.getAttribute('normal'), undefined);
  assert.deepEqual(
    [g.getAttribute('position').count, g.index.count],
    [8, 30],
    'four walls and a lid',
  );
  const two = new MeshBuilder({ compact: true });
  two.quad([0, 0, 0], [4, 0, 0], [4, 3, 0], [0, 3, 0], new THREE.Color(0x56794d), [0, 0, 1]);
  two.quad([4, 0, 0], [8, 0, 0], [8, 3, 0], [4, 3, 0], new THREE.Color(0x56794d), [0, 0, 1]);
  two.glow.set(0xffcf83);
  two.quad([8, 0, 0], [12, 0, 0], [12, 3, 0], [8, 3, 0], new THREE.Color(0x56794d), [0, 0, 1]);
  assert.equal(two.vertexCount, 10, 'a glowing face keeps its own corners');
});

test('compact meshes draw the same triangles in the same order', () => {
  for (let zone = 1; zone <= 6; zone++) {
    const lot = rect(0, 0, 28, 40);
    const building = { zone, height: 12, roof: zone % 4, front: 0, ring: rect(7, 6, 14, 14) };
    const [plain, shared] = [new MeshBuilder(), new MeshBuilder({ compact: true })];
    for (const mb of [plain, shared]) lotGeometry(mb, lot, building, new Clearance());
    assert.deepEqual(triangles(shared.geometry()), triangles(plain.geometry()), `zone ${zone}`);
    assert.ok(shared.vertexCount < plain.vertexCount * 0.75, `zone ${zone} shares corners`);
  }
});

test('compact buildings and lots fit tighter memory budgets', () => {
  const data = [];
  for (let zone = 1; zone <= 6; zone++) data.push(...record(zone, rect(zone * 40, 0, 18, 14)));
  const g = buildBuildingGeometry(new Float32Array(data), undefined, { compact: true });
  assert.equal(g.getAttribute('normal'), undefined);
  assert.ok(g.getAttribute('lightOn'), 'some buildings keep evening hours');
  assert.ok(bytesPerVertex(g) <= 20, `${bytesPerVertex(g)} bytes per building vertex`);
  const mb = new MeshBuilder({ compact: true });
  for (let zone = 1; zone <= 6; zone++) {
    const lot = rect(zone * 40, 0, 28, 40);
    const building = { zone, height: 6, roof: 0, front: 0, ring: rect(zone * 40 + 7, 6, 14, 14) };
    lotGeometry(mb, lot, building, new Clearance());
  }
  const lots = mb.geometry();
  assert.ok(bytesPerVertex(lots) <= 16, `${bytesPerVertex(lots)} bytes per lot vertex`);
  assert.ok(bytesPerTriangle(lots) <= 24, `${bytesPerTriangle(lots)} bytes per lot triangle`);
});

test('meshes with at most 65,536 corners index them with 16-bit entries', () => {
  const small = new MeshBuilder();
  small.quad([0, 0, 0], [4, 0, 0], [4, 3, 0], [0, 3, 0], new THREE.Color(0x56794d), [0, 0, 1]);
  assert.ok(small.geometry().index.array instanceof Uint16Array);
  const large = new MeshBuilder();
  for (let i = 0; i < 16385; i++)
    large.quad([i, 0, 0], [i + 0.5, 0, 0], [i + 0.5, 1, 0], [i, 1, 0], new THREE.Color(0xffffff));
  assert.equal(large.vertexCount, 65540);
  const index = large.geometry().index.array;
  assert.ok(index instanceof Uint32Array);
  assert.equal(index.at(-1), 65539, 'the last corner keeps its full index');
});
