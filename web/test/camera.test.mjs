import assert from 'node:assert/strict';
import test from 'node:test';
import CameraControls from 'camera-controls';
import * as THREE from 'three';
import { panAlongGround } from '../src/camera.ts';

CameraControls.install({ THREE });
globalThis.DOMRect ??= class {
  x = 0;
  y = 0;
  width = 0;
  height = 0;
  left = 0;
  top = 0;
};

function controls(azimuth, polar) {
  const noop = () => {};
  const element = {
    style: {},
    addEventListener: noop,
    removeEventListener: noop,
    setAttribute: noop,
    ownerDocument: { addEventListener: noop, removeEventListener: noop },
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 800, height: 600 }),
  };
  const camera = new THREE.PerspectiveCamera(45, 4 / 3, 1, 50000);
  const c = new CameraControls(camera, element);
  c.setLookAt(100, 800, 300, 100, 0, -200, false);
  c.rotateTo(azimuth, polar, false);
  c.update(0);
  camera.updateMatrixWorld();
  return { c, camera };
}

test('keyboard panning stays on the ground at any orbit and pitch', () => {
  for (const [azimuth, polar] of [
    [0, 0.3],
    [1.1, 1.2],
    [-2.5, 0.01],
    [0.4, THREE.MathUtils.degToRad(70)],
  ]) {
    const { c, camera } = controls(azimuth, polar);
    const distance = c.distance;
    const height = camera.position.y;
    const start = c.getTarget(new THREE.Vector3());
    const screenRight = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 0);
    const look = camera.getWorldDirection(new THREE.Vector3()).setY(0).normalize();
    panAlongGround(c, 0.03, 0.04);
    c.update(10);
    const moved = c.getTarget(new THREE.Vector3()).sub(start);
    assert.ok(Math.abs(camera.position.y - height) < 1e-6, 'height unchanged');
    assert.ok(Math.abs(c.distance - distance) < 1e-6, 'zoom unchanged');
    assert.ok(Math.abs(moved.y) < 1e-6, 'target stays on the ground');
    assert.ok(Math.abs(moved.length() - 0.05 * distance) < 1e-6, 'speed follows zoom');
    assert.ok(Math.abs(moved.dot(screenRight) - 0.03 * distance) < 1e-6, 'D moves right');
    assert.ok(Math.abs(moved.dot(look) - 0.04 * distance) < 1e-6, 'W moves where it looks');
  }
});

test('a long keyboard pan leaves no pending camera transitions behind', () => {
  const { c } = controls(0.4, 0.8);
  let waiting = 0;
  const add = c.addEventListener.bind(c);
  c.addEventListener = (type, listener) => {
    if (type === 'rest') waiting++;
    add(type, listener);
  };
  for (let frame = 0; frame < 1000; frame++) {
    panAlongGround(c, 0.001, 0.002);
    c.update(1 / 144);
  }
  assert.equal(waiting, 0);
});

test('after a mouse pan settles, keyboard panning stops adding camera transitions', () => {
  const { c } = controls(0.4, 0.8);
  let waiting = 0;
  const add = c.addEventListener.bind(c);
  c.addEventListener = (type, listener) => {
    if (type === 'rest') waiting++;
    add(type, listener);
  };
  c.truck(200, 0, true);
  for (let frame = 0; frame < 144 * 5; frame++) {
    panAlongGround(c, 0.001, 0.002);
    c.update(1 / 144);
  }
  const settled = waiting;
  assert.ok(settled > 0 && settled < 144 * 5, 'transitions only while the mouse pan settles');
  for (let frame = 0; frame < 1000; frame++) {
    panAlongGround(c, 0.001, 0.002);
    c.update(1 / 144);
  }
  assert.equal(waiting, settled);
});

test('keyboard panning keeps an unfinished mouse pan gliding instead of jumping', () => {
  const { c, camera } = controls(0.4, 0.8);
  const screenRight = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 0);
  c.truck(200, 0, true); // what a mouse drag leaves behind: a target still being eased toward
  c.update(1 / 60);
  const now = c.getTarget(new THREE.Vector3(), false);
  const gap = c.getTarget(new THREE.Vector3(), true).sub(now);
  assert.ok(gap.length() > 50, 'the mouse pan is still settling');
  panAlongGround(c, 0, 0.01);
  const moved = c.getTarget(new THREE.Vector3(), false).sub(now);
  assert.ok(Math.abs(moved.dot(screenRight)) < 1e-6, 'no sideways jump to the mouse pan end');
  assert.ok(Math.abs(moved.length() - 0.01 * c.distance) < 1e-6, 'only the keyboard step');
  const after = c.getTarget(new THREE.Vector3(), true).sub(c.getTarget(new THREE.Vector3(), false));
  assert.ok(after.distanceTo(gap) < 1e-6, 'the rest of the mouse pan still eases in');
});
