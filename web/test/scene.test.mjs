import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { createSimScene } from '../src/scene.ts';

test('simulation right-hand lanes remain on the right in the 3D view', () => {
  const scene = createSimScene();
  for (const heading of [0, Math.PI / 2, Math.PI, -Math.PI / 2, 0.7]) {
    const forward = new THREE.Vector3(Math.cos(heading), 0, Math.sin(heading));
    const laneOffset = new THREE.Vector3(Math.sin(heading) * 2, 0, -Math.cos(heading) * 2);
    forward.transformDirection(scene.matrixWorld);
    const right = forward.clone().cross(new THREE.Vector3(0, 1, 0));
    scene.localToWorld(laneOffset);
    assert.ok(laneOffset.dot(right) > 1.99);
  }
});

test('roads, cars, signals and previews share a projection that picking can invert', () => {
  const scene = createSimScene();
  const point = new THREE.Vector3(12, 0, 34);
  assert.deepEqual(scene.localToWorld(point.clone()).toArray(), [12, 0, -34]);
  for (const height of [0, 0.5, 1, 4]) {
    const object = new THREE.Object3D();
    object.position.set(12, height, 34);
    scene.add(object);
    const world = object.getWorldPosition(new THREE.Vector3());
    assert.deepEqual(world.toArray(), [12, height, -34]);
    assert.deepEqual(scene.worldToLocal(world).toArray(), [12, height, 34]);
  }
});
