// Growable instanced meshes for things drawn many times per frame: cars, walkers, signals.
// Only the drawn instances are uploaded after a change (`drawFirst`).

import * as THREE from 'three';

/**
 * Returns `mesh` if it can hold `count` instances, else a bigger replacement added to `scene`
 * with a fresh `geometry()` and the same `material`. `mesh.count` is how many instances were
 * drawn last frame; `instanceMatrix.count` is capacity.
 */
export function ensureInstances(
  scene: THREE.Object3D,
  mesh: THREE.InstancedMesh | undefined,
  count: number,
  geometry: () => THREE.BufferGeometry,
  material: THREE.Material,
): THREE.InstancedMesh {
  if (mesh && mesh.instanceMatrix.count >= count) return mesh;
  if (mesh) {
    scene.remove(mesh);
    mesh.dispose();
    mesh.geometry.dispose();
  }
  const capacity = Math.ceil(Math.max(count, 1024) * 1.5);
  const instances = new THREE.InstancedMesh(geometry(), material, capacity);
  instances.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
  instances.frustumCulled = false;
  scene.add(instances);
  return instances;
}

/**
 * Draws the first `count` instances of `mesh` and marks only those for upload. three.js
 * otherwise sends the whole capacity on every change, even with nothing to draw.
 */
export function drawFirst(mesh: THREE.InstancedMesh, count: number) {
  mesh.count = count;
  if (count === 0) return;
  for (const attribute of [mesh.instanceMatrix, mesh.instanceColor]) {
    if (!attribute) continue;
    attribute.clearUpdateRanges();
    attribute.addUpdateRange(0, count * attribute.itemSize);
    attribute.needsUpdate = true;
  }
}
