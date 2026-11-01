import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { carPosition, nearestCar } from '../src/car-pick.ts';
import { createSimScene } from '../src/scene.ts';

const W = 800;
const H = 600;
/** A camera looking straight down at sim (0, 0) from 500 m. */
function view() {
  const scene = createSimScene();
  scene.updateMatrixWorld();
  const camera = new THREE.PerspectiveCamera(45, W / H, 1, 50000);
  camera.position.set(0, 500, 0);
  camera.up.set(0, 0, -1);
  camera.lookAt(0, 0, 0);
  camera.updateMatrixWorld();
  return { scene, camera };
}
/** Screen pixel of sim point (x, y). */
function screen(scene, camera, x, y) {
  const p = scene.localToWorld(new THREE.Vector3(x, 0, y)).project(camera);
  return [((p.x + 1) / 2) * W, ((1 - p.y) / 2) * H];
}

test('a click picks the drawn car under it by agent id, skipping hidden slots', () => {
  const { scene, camera } = view();
  // Agent 0 hidden, 1 at (20, 10), 2 at (-40, 0); the snapshot index is the agent id.
  const drawn = new Float32Array([Number.NaN, Number.NaN, 20, 10, -40, 0]);
  const [x, y] = screen(scene, camera, 20, 10);
  assert.equal(nearestCar(drawn, scene, camera, x + 3, y - 3, W, H), 1);
  const [hx, hy] = screen(scene, camera, -40, 0);
  assert.equal(nearestCar(drawn, scene, camera, hx, hy, W, H), 2);
  assert.equal(nearestCar(drawn, scene, camera, x + 40, y, W, H), null, 'too far is a miss');
  drawn[2] = Number.NaN;
  assert.equal(nearestCar(drawn, scene, camera, x, y, W, H), null, 'hidden cars are not picked');
});

test('the nearest of two close cars wins', () => {
  const { scene, camera } = view();
  const drawn = new Float32Array([0, 0, 2, 0]);
  const [x, y] = screen(scene, camera, 1.8, 0);
  assert.equal(nearestCar(drawn, scene, camera, x, y, W, H), 1);
});

test('car positions use the scene projection and refuse unknown or hidden ids', () => {
  const { scene } = view();
  const drawn = new Float32Array([12, 34, Number.NaN, Number.NaN]);
  const out = new THREE.Vector3();
  assert.equal(carPosition(drawn, scene, 0, out), true);
  assert.deepEqual(out.toArray(), [12, 0, -34]);
  assert.equal(carPosition(drawn, scene, 1, out), false);
  assert.equal(carPosition(drawn, scene, 2, out), false);
});
