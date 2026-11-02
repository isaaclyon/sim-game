// Growable instanced meshes for things drawn many times per frame: cars, walkers, signals.

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

/** A box `size` long (x), tall (y) and wide (z), standing on the ground. */
export function standingBox(size: [number, number, number]): THREE.BufferGeometry {
  const geom = new THREE.BoxGeometry(...size);
  geom.translate(0, size[1] / 2, 0);
  return geom;
}
