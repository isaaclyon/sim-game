import * as THREE from 'three';

/** Children use sim (x, y) as local (x, z). Reflect z once at the scene root so
 * sim +y is world -z, preserving right-hand traffic when viewed from above.
 * Picking must use worldToLocal to return coordinates to the simulation.
 */
export function createSimScene(): THREE.Scene {
  const scene = new THREE.Scene();
  scene.scale.z = -1;
  scene.updateMatrixWorld(true);
  return scene;
}
