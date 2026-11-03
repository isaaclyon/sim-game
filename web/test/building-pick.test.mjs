import assert from 'node:assert/strict';
import test from 'node:test';
import * as THREE from 'three';
import { BuildingLayer } from '../src/building-kit/layer.ts';
import { createSimScene } from '../src/scene.ts';

const house = (x, height = 6) => [1, height, 0, 0, 4, x, 0, x + 12, 0, x + 12, 10, x, 10];
const apartment = (x) => [2, 30, 0, 0, 4, x, 0, x + 12, 0, x + 12, 10, x, 10];

function setup() {
  const layer = new BuildingLayer(new THREE.MeshBasicMaterial());
  const scene = createSimScene();
  scene.add(layer.group);
  const pick = (localOrigin, localTarget) => {
    scene.updateMatrixWorld(true);
    const origin = scene.localToWorld(new THREE.Vector3(...localOrigin));
    const target = scene.localToWorld(new THREE.Vector3(...localTarget));
    return layer.pick(new THREE.Raycaster(origin, target.sub(origin).normalize()));
  };
  return { layer, pick };
}

test('picking roofs and facades returns stable IDs, not packed indices or tiles', () => {
  const { layer, pick } = setup();
  layer.update(
    new Float32Array([...house(0), ...apartment(40), ...house(1000)]),
    new Uint32Array([8, 21, 79]),
  );
  assert.equal(pick([6, 100, 5], [6, 0, 5]), 8);
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 21);
  assert.equal(pick([1006, 100, 5], [1006, 0, 5]), 79);
  // A facade click projects well behind the footprint if tested against the ground instead.
  assert.equal(pick([46, 20, -30], [46, 20, 5]), 21);
  assert.equal(pick([300, 100, 5], [300, 0, 5]), null);
  layer.update(new Float32Array([...apartment(40), ...house(1000)]), new Uint32Array([21, 79]));
  assert.equal(pick([6, 100, 5], [6, 0, 5]), null, 'deleted buildings stop being pickable');
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 21, 'packing after deletion cannot shift identity');
  // Even unchanged geometry may represent new IDs after demolition and rebuilding.
  layer.update(new Float32Array([...apartment(40), ...house(1000)]), new Uint32Array([83, 79]));
  assert.equal(pick([46, 100, 5], [46, 0, 5]), 83);
  layer.update(new Float32Array(), new Uint32Array());
  assert.equal(pick([46, 100, 5], [46, 0, 5]), null);
});
