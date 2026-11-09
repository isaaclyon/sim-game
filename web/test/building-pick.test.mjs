import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { buildBuildingGeometry } from '../src/building-kit/index.ts';
import { BuildingLayer } from '../src/building-kit/layer.ts';
import { createSimScene } from '../src/scene.ts';

const house = (x, height = 6) => [1, height, 0, 0, 1, 4, x, 0, x + 12, 0, x + 12, 10, x, 10];
const apartment = (x) => [2, 30, 0, 0, 1, 4, x, 0, x + 12, 0, x + 12, 10, x, 10];

/** What the renderer does when it first draws a mesh: hands each vertex array to the GPU. */
function upload(group) {
  group.traverse((o) => {
    if (o.isMesh)
      for (const a of Object.values(o.geometry.attributes)) (a.data ?? a).onUploadCallback();
  });
}
const arrays = (group) =>
  group.children.flatMap((m) =>
    Object.values(m.geometry.attributes).map((a) => (a.data ?? a).array),
  );

function setup() {
  const layer = new BuildingLayer(new THREE.MeshBasicMaterial());
  const scene = createSimScene();
  scene.add(layer.group);
  const update = (data, ids) => {
    layer.update(new Float32Array(data), new Uint32Array(ids));
    upload(layer.group);
  };
  const ray = (localOrigin, localTarget) => {
    scene.updateMatrixWorld(true);
    const origin = scene.localToWorld(new THREE.Vector3(...localOrigin));
    const target = scene.localToWorld(new THREE.Vector3(...localTarget));
    return new THREE.Raycaster(origin, target.sub(origin).normalize());
  };
  const pick = (localOrigin, localTarget) => layer.pick(ray(localOrigin, localTarget));
  return { layer, scene, update, ray, pick };
}

test('picking roofs and facades returns stable IDs, not packed indices or tiles', () => {
  const { update, pick } = setup();
  update([...house(0), ...apartment(40), ...house(1000)], [8, 21, 79]);
  assert.equal(pick([6, 100, 5], [6, 0, 5]), 8);
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 21);
  assert.equal(pick([1006, 100, 5], [1006, 0, 5]), 79);
  // A facade click projects well behind the footprint if tested against the ground instead.
  assert.equal(pick([46, 20, -30], [46, 20, 5]), 21);
  assert.equal(pick([300, 100, 5], [300, 0, 5]), null);
  update([...apartment(40), ...house(1000)], [21, 79]);
  assert.equal(pick([6, 100, 5], [6, 0, 5]), null, 'deleted buildings stop being pickable');
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 21, 'packing after deletion cannot shift identity');
  // Even unchanged geometry may represent new IDs after demolition and rebuilding.
  update([...apartment(40), ...house(1000)], [83, 79]);
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 83);
  update([], []);
  assert.equal(pick([46, 100, 5], [46, 0, 5]), null);
});

/** Every use and roof, turned at odd angles, across several tiles. */
function city() {
  const records = [];
  for (let i = 0; i < 36; i++) {
    const [cx, cy] = [20 + (i % 9) * 66, 20 + Math.floor(i / 9) * 70];
    const [w, d, a] = [10 + (i % 4) * 4, 9 + (i % 3) * 5, i * 0.37];
    const ring = [
      [-w, -d],
      [w, -d],
      [w, d],
      [-w, d],
    ].map(([x, y]) => [
      cx + (x * Math.cos(a) - y * Math.sin(a)) / 2,
      cy + (x * Math.sin(a) + y * Math.cos(a)) / 2,
    ]);
    records.push(1 + (i % 6), 5 + (i % 7) * 6, i % 4, i % 4, 1, 4, ...ring.flat());
  }
  return new Float32Array(records);
}

test('picking after tiles drop their vertices matches raycasting the drawn buildings', () => {
  const { layer, scene, update, ray } = setup();
  const data = city();
  const ids = Array.from({ length: 36 }, (_, i) => 100 + i);
  update(data, ids);
  assert.ok(
    arrays(layer.group).every((a) => a === null),
    'tiles keep no vertices once drawn',
  );
  // The old way: raycast every drawn triangle, then map the face to its building.
  const whole = new THREE.Mesh(buildBuildingGeometry(data), new THREE.MeshBasicMaterial());
  scene.add(whole);
  const expected = (r) => {
    const hit = r.intersectObject(whole)[0];
    if (!hit) return null;
    const vertex = hit.faceIndex * 3;
    return ids[
      whole.geometry.groups.findIndex((g) => vertex >= g.start && vertex < g.start + g.count)
    ];
  };
  let seed = 7;
  const random = () => {
    seed = (seed * 1103515245 + 12345) % 2 ** 31;
    return seed / 2 ** 31;
  };
  let hits = 0;
  for (let i = 0; i < 400; i++) {
    const r = ray(
      [-50 + random() * 700, 5 + random() * 300, -60 + random() * 380],
      // Near a building's middle, so most rays hit one and some pass others first.
      [
        20 + Math.floor(random() * 9) * 66 + random() * 12 - 6,
        random() * 45,
        20 + Math.floor(random() * 4) * 70 + random() * 12 - 6,
      ],
    );
    const want = expected(r);
    if (want !== null) hits++;
    assert.equal(layer.pick(r), want, `ray ${i}`);
  }
  assert.ok(hits > 200, `${hits} of 400 rays hit a building`);
});

test('tiles rebuild with fresh vertices when the GPU loses them', () => {
  const { layer, update, pick } = setup();
  update([...house(0), ...apartment(40)], [8, 21]);
  const before = layer.group.children[0];
  layer.restore();
  assert.notEqual(layer.group.children[0], before);
  assert.ok(
    arrays(layer.group).every((a) => a !== null),
    'arrays to upload again',
  );
  upload(layer.group);
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 21);
});
