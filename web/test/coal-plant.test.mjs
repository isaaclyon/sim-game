import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { createCoalPlant } from '../src/coal-plant.ts';
import { PowerLayer } from '../src/power-layer.ts';
import { PlantKind } from '../src/protocol.ts';

test('the coal model fits its reserved land and remains inexpensive to draw', () => {
  const plant = createCoalPlant(22);
  const bounds = new THREE.Box3().setFromObject(plant);
  assert.equal(bounds.min.y, 0);
  assert.ok(bounds.max.y >= 40 && bounds.max.y <= 45, 'the chimney keeps its skyline height');
  assert.ok(bounds.min.x >= -22 && bounds.max.x <= 22);
  assert.ok(bounds.min.z >= -22 && bounds.max.z <= 22, 'no equipment in the road or terminal');
  let meshes = 0;
  let triangles = 0;
  plant.traverse((object) => {
    if (!object.isMesh) return;
    meshes++;
    const geometry = object.geometry;
    const positions = geometry.getAttribute('position');
    const normals = geometry.getAttribute('normal');
    const colors = geometry.getAttribute('color');
    assert.equal(positions.count, normals.count);
    assert.equal(positions.count, colors.count);
    for (let i = 0; i < positions.count; i++) {
      assert.ok([positions.getX(i), positions.getY(i), positions.getZ(i)].every(Number.isFinite));
      assert.ok(Math.abs(Math.hypot(normals.getX(i), normals.getY(i), normals.getZ(i)) - 1) < 1e-5);
    }
    triangles += positions.count / 3;
    assert.ok(object.castShadow && object.receiveShadow);
  });
  assert.equal(meshes, 1, 'details share a single draw call');
  assert.ok(triangles < 6000, `${triangles} triangles`);
});

test('repeated power updates reuse coal geometry and preserve road orientation and terminal', () => {
  const layer = new PowerLayer();
  const snapshot = new Float32Array([PlantKind.Coal, 100, 200, 22, -1, 0, 124, 200, 3000, 0]);
  const empty = new Float32Array();
  const coalMesh = () => {
    let found;
    layer.group.traverse((object) => {
      if (object.isMesh && object.material.vertexColors) found = object;
    });
    return found;
  };
  layer.set(snapshot, empty, empty);
  const first = coalMesh();
  assert.ok(first);
  const geometry = first.geometry;
  const material = first.material;
  layer.group.updateMatrixWorld(true);
  const front = new THREE.Vector3(0, 0, 22).applyMatrix4(first.matrixWorld);
  assert.ok(front.distanceTo(new THREE.Vector3(78, 0, 200)) < 1e-5, 'front faces the road');
  const terminal = layer.group.children[0].children[1];
  assert.equal(terminal.position.x, 124);
  assert.equal(terminal.position.z, 200);
  layer.set(snapshot, empty, empty);
  assert.equal(coalMesh().geometry, geometry, 'no new GPU buffers on each power update');
  assert.equal(coalMesh().material, material);
  layer.set(empty, empty, empty);
  assert.equal(coalMesh(), undefined, 'demolition removes the model');
});
