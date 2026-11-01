import assert from 'node:assert/strict';
import test from 'node:test';
import CameraControls from 'camera-controls';
import * as THREE from 'three';
import {
  KEY_EASE,
  KEY_PAN_SPEED,
  KEY_TURN_SPEED,
  KeyCamera,
  panAlongGround,
  turnAroundTarget,
} from '../src/camera.ts';

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

const target = (c) => c.getTarget(new THREE.Vector3(), false);
/** Runs `frames` frames of `seconds` with the given keys; returns the target's total move. */
function hold(c, keys, keyCamera, frames, seconds) {
  const start = target(c);
  for (let i = 0; i < frames; i++) {
    keyCamera.update(...keys, seconds);
    c.update(seconds);
  }
  return target(c).sub(start);
}

test('held keys speed up along a curve and glide to a stop when released', () => {
  const { c } = controls(0.4, 0.8);
  const keys = new KeyCamera(c);
  const top = KEY_PAN_SPEED * c.distance;
  const steps = [];
  for (let i = 0; i < 60; i++) steps.push(hold(c, [0, 1, 0], keys, 1, 1 / 60).length() * 60);
  assert.ok(steps[0] < top * 0.1, 'starts slowly');
  assert.ok(
    steps.every((v, i) => i === 0 || v > steps[i - 1]),
    'speeds up every frame',
  );
  assert.ok(steps.at(-1) > top * 0.98 && steps.at(-1) <= top, 'reaches top speed in a second');
  const glide = hold(c, [0, 0, 0], keys, 120, 1 / 60).length();
  assert.ok(Math.abs(glide - top * KEY_EASE) < top * 0.01, 'glides about one ease time');
  assert.equal(keys.update(0, 0, 0, 1 / 60), false, 'then stops completely');
});

test('keyboard motion follows the same path at any frame rate, without stall jumps', () => {
  const a = controls(0.4, 0.8);
  const b = controls(0.4, 0.8);
  const slow = hold(a.c, [1, 1, 0], new KeyCamera(a.c), 30, 1 / 30);
  const fast = hold(b.c, [1, 1, 0], new KeyCamera(b.c), 144, 1 / 144);
  assert.ok(slow.distanceTo(fast) < 1e-6 * a.c.distance);
  const straight = controls(0.4, 0.8);
  const ahead = hold(straight.c, [0, 1, 0], new KeyCamera(straight.c), 144, 1 / 144);
  assert.ok(Math.abs(fast.length() - ahead.length()) < 1e-6, 'diagonals are not faster');
  const stalled = controls(0.4, 0.8);
  const keys = new KeyCamera(stalled.c);
  const once = hold(stalled.c, [0, 1, 0], keys, 1, 5).length();
  assert.ok(once < KEY_PAN_SPEED * stalled.c.distance * 0.1, 'a stall moves at most 0.1 s');
  keys.stop();
  assert.equal(keys.update(0, 0, 0, 1 / 60), false, 'stop ends the glide at once');
});

test('Q and E turn the view left and right around its target, easing in', () => {
  for (const [turn, sign] of [
    [1, 1],
    [-1, -1],
  ]) {
    const { c, camera } = controls(0.4, 0.8);
    const left = new THREE.Vector3().setFromMatrixColumn(camera.matrixWorld, 0).negate();
    const [start, polar, distance] = [target(c), c.polarAngle, c.distance];
    const azimuth = c.azimuthAngle;
    const keys = new KeyCamera(c);
    keys.update(0, 0, turn, 1 / 60);
    c.update(1 / 60);
    const first = c.azimuthAngle - azimuth;
    hold(c, [0, 0, turn], keys, 119, 1 / 60);
    camera.updateMatrixWorld();
    const look = camera.getWorldDirection(new THREE.Vector3()).setY(0).normalize();
    assert.ok(Math.sign(look.dot(left)) === sign, turn > 0 ? 'Q turns left' : 'E turns right');
    assert.ok(Math.abs(first) < (KEY_TURN_SPEED / 60) * 0.2, 'turning starts slowly');
    const turned = Math.abs(c.azimuthAngle - azimuth);
    assert.ok(turned > KEY_TURN_SPEED * 1.7 && turned < KEY_TURN_SPEED * 2, 'then turns steadily');
    assert.ok(target(c).distanceTo(start) < 1e-6, 'around the point it looks at');
    assert.ok(Math.abs(c.polarAngle - polar) < 1e-9 && Math.abs(c.distance - distance) < 1e-6);
  }
});

test('Q and E keep an unfinished mouse orbit easing instead of jumping', () => {
  const { c } = controls(0.4, 0.8);
  c.rotate(1, 0.2, true);
  c.update(1 / 60);
  const now = c.azimuthAngle;
  const end = c.getSpherical(new THREE.Spherical(), true);
  turnAroundTarget(c, 0.01);
  assert.ok(Math.abs(c.azimuthAngle - now - 0.01) < 1e-9, 'only the key step is applied now');
  const after = c.getSpherical(new THREE.Spherical(), true);
  assert.ok(Math.abs(after.theta - end.theta - 0.01) < 1e-9, 'the orbit still eases in');
  assert.ok(Math.abs(after.phi - end.phi) < 1e-9, 'and so does its tilt');
});
