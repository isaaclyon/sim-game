import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { AgentLayer } from '../src/agent-layer.ts';
import { drawFirst, ensureInstances } from '../src/instances.ts';
import { AGENT_STRIDE } from '../src/protocol.ts';

const cube = () => new THREE.BoxGeometry(1, 1, 1);
const material = new THREE.MeshBasicMaterial();
/** Floats marked for upload, as three.js sends them: the whole array when no range is set. */
const marked = (a) =>
  a.updateRanges.length === 0 ? a.array.length : a.updateRanges.reduce((n, r) => n + r.count, 0);

test('drawFirst uploads only the drawn instances, not the whole capacity', () => {
  const mesh = ensureInstances(new THREE.Group(), undefined, 10, cube, material);
  mesh.setColorAt(0, new THREE.Color(1, 0, 0));
  const [matrix, color] = [mesh.instanceMatrix.version, mesh.instanceColor.version];
  drawFirst(mesh, 10);
  assert.equal(mesh.count, 10);
  assert.ok(mesh.instanceMatrix.version > matrix && mesh.instanceColor.version > color);
  assert.equal(marked(mesh.instanceMatrix), 10 * 16);
  assert.equal(marked(mesh.instanceColor), 10 * 3);
  assert.ok(mesh.instanceMatrix.count > 1000, 'capacity is far above the drawn count');
});

test('drawFirst uploads nothing when nothing is drawn', () => {
  const mesh = ensureInstances(new THREE.Group(), undefined, 10, cube, material);
  drawFirst(mesh, 4);
  const version = mesh.instanceMatrix.version;
  drawFirst(mesh, 0);
  assert.equal(mesh.count, 0);
  assert.equal(mesh.instanceMatrix.version, version);
});

test('agents upload only the cars drawn this frame', () => {
  const scene = new THREE.Group();
  const layer = new AgentLayer(scene, material);
  const camera = new THREE.PerspectiveCamera(50, 1, 1, 5000);
  camera.position.set(0, 400, 1);
  camera.lookAt(0, 0, 0);
  const cars = new Float32Array(3 * AGENT_STRIDE);
  for (let i = 0; i < 3; i++) cars.set([i * 10, 0, 0, 1, 0], i * AGENT_STRIDE);
  layer.draw(undefined, cars, 1, null, camera, 2);
  const meshes = scene.children.filter((m) => m.isInstancedMesh);
  const floats = meshes.reduce((n, m) => n + (m.count ? marked(m.instanceMatrix) : 0), 0);
  const drawn = meshes.reduce((n, m) => n + m.count, 0);
  assert.ok(drawn >= 3, 'every car is drawn');
  assert.equal(floats, drawn * 16, 'only drawn matrices are uploaded');
  const idle = meshes.filter((m) => m.count === 0 && m.instanceMatrix.version > 0);
  assert.deepEqual(idle, [], 'empty meshes upload nothing');
});

test('agents skip the redraw when nothing they show has changed', () => {
  const scene = new THREE.Group();
  const layer = new AgentLayer(scene, material);
  const camera = new THREE.PerspectiveCamera(50, 1, 1, 5000);
  camera.position.set(0, 400, 1);
  camera.lookAt(0, 0, 0);
  const cars = new Float32Array([0, 0, 0, 1, 0, 10, 0, 0, 1, 0]);
  const versions = () =>
    scene.children.filter((m) => m.isInstancedMesh).map((m) => m.instanceMatrix.version);
  layer.draw(undefined, cars, 1, null, camera, 2);
  const first = versions();
  layer.draw(undefined, cars, 1, null, camera, 2);
  assert.deepEqual(versions(), first, 'a paused, still view uploads nothing');
  camera.position.x += 5;
  layer.draw(undefined, cars, 1, null, camera, 2);
  const moved = versions();
  assert.notDeepEqual(moved, first, 'a camera move redraws');
  layer.draw(undefined, cars, 1, 0, camera, 2);
  assert.notDeepEqual(versions(), moved, 'selecting a car redraws');
  const next = versions();
  layer.draw(cars, cars.slice(), 0.5, 0, camera, 2);
  assert.notDeepEqual(versions(), next, 'a new snapshot redraws');
});
