import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { PowerLayer } from '../src/power-layer.ts';
import { PlantKind } from '../src/protocol.ts';
import { createSolarPlant } from '../src/solar-plant.ts';
import { createWindPlant } from '../src/wind-plant.ts';

function meshes(object) {
  const result = [];
  object.traverse((child) => {
    if (child.isMesh) result.push(child);
  });
  return result;
}

function validGeometry(object, calls, budget) {
  const parts = meshes(object);
  assert.equal(parts.length, calls);
  let triangles = 0;
  for (const mesh of parts) {
    assert.ok(mesh.castShadow && mesh.receiveShadow);
    const p = mesh.geometry.getAttribute('position');
    const n = mesh.geometry.getAttribute('normal');
    assert.equal(mesh.geometry.getAttribute('color').count, p.count);
    triangles += p.count / 3;
    for (let i = 0; i < p.count; i++) {
      assert.ok([p.getX(i), p.getY(i), p.getZ(i)].every(Number.isFinite));
      // Normals are stored as bytes: unit length to within 1%.
      assert.ok(Math.abs(Math.hypot(n.getX(i), n.getY(i), n.getZ(i)) - 1) < 0.01);
    }
  }
  assert.ok(triangles < budget, `${triangles} triangles must stay under ${budget}`);
}

test('solar detail stays inside its footprint and uses a single shared mesh', () => {
  const solar = createSolarPlant(16);
  validGeometry(solar, 1, 6000);
  const { min, max } = new THREE.Box3().setFromObject(solar);
  assert.equal(min.y, 0);
  assert.ok(max.y < 5);
  assert.ok(min.x >= -16 && max.x <= 16 && min.z >= -16 && max.z <= 16);
  const other = createSolarPlant(16);
  assert.equal(other.geometry, solar.geometry);
  assert.equal(other.material, solar.material);
});

test('wind equipment fits its land and the blades clear the tower throughout rotation', () => {
  const { group, rotor } = createWindPlant(5);
  validGeometry(group, 2, 2500);
  const body = group.children.find((child) => child !== rotor);
  const bounds = new THREE.Box3().setFromObject(body);
  assert.equal(bounds.min.y, 0);
  assert.ok(bounds.min.x >= -5 && bounds.max.x <= 5);
  assert.ok(bounds.min.z >= -5 && bounds.max.z <= 5);
  for (let step = 0; step < 12; step++) {
    rotor.rotation.z = (step * Math.PI) / 6;
    const swept = new THREE.Box3().setFromObject(rotor, true);
    assert.ok(swept.min.y > 20 && swept.max.y <= 48, 'same 34 m hub and 13 m blade reach');
    assert.ok(swept.min.z > 1.5, 'the rotor stays ahead of the housing’s front face');
  }
  const other = createWindPlant(5);
  const first = meshes(group);
  const second = meshes(other.group);
  for (let i = 0; i < first.length; i++) {
    assert.equal(first[i].geometry, second[i].geometry);
    assert.equal(first[i].material, second[i].material);
  }
  assert.notEqual(rotor, other.rotor, 'each turbine rotates independently');
});

test('power snapshots keep terminals and output-driven wind animation intact', () => {
  const layer = new PowerLayer();
  const empty = new Float32Array();
  const snapshot = new Float32Array([
    PlantKind.Solar,
    50,
    60,
    16,
    0,
    1,
    50,
    42,
    800,
    0,
    PlantKind.Wind,
    100,
    200,
    5,
    0,
    1,
    100,
    193,
    500,
    0,
    PlantKind.Wind,
    120,
    200,
    5,
    0,
    1,
    120,
    193,
    250,
    0,
  ]);
  layer.set(snapshot, empty, empty);
  const plants = layer.group.children[0];
  for (const [index, x, z] of [
    [1, 50, 42],
    [3, 100, 193],
    [5, 120, 193],
  ]) {
    assert.equal(plants.children[index].position.x, x);
    assert.equal(plants.children[index].position.z, z);
  }
  const before = meshes(plants).map((mesh) => mesh.geometry);
  const fastRotor = plants.children[2].children[0].children[1];
  const slowRotor = plants.children[4].children[0].children[1];
  layer.animate(1);
  assert.ok(Math.abs(fastRotor.rotation.z - 0.8 * Math.PI) < 1e-8);
  assert.ok(Math.abs(slowRotor.rotation.z - 0.4 * Math.PI) < 1e-8);
  layer.set(snapshot, empty, empty);
  assert.deepEqual(
    meshes(plants).map((mesh) => mesh.geometry),
    before,
  );
  layer.set(empty, empty, empty);
  layer.animate(2);
  assert.equal(plants.children.length, 0, 'demolition clears the models and animation');
});
