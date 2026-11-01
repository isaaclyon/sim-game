// Picking drawn cars for the citizen inspector. Works on the renderer's per-agent positions,
// so it finds the car where the user sees it, interpolated, with no per-click allocation.

import * as THREE from 'three';

/** A click this close to a drawn car, in CSS pixels, selects it. */
export const PICK_PX = 16;

/**
 * Writes the world position of car `agent` into `out` from `drawn` (`[x, y]` per agent id in
 * scene-local sim coordinates, NaN when hidden). False if the car is hidden or unknown.
 */
export function carPosition(
  drawn: Float32Array,
  scene: THREE.Object3D,
  agent: number,
  out: THREE.Vector3,
): boolean {
  const x = agent * 2 + 1 < drawn.length ? drawn[agent * 2] : Number.NaN;
  if (Number.isNaN(x)) return false;
  scene.localToWorld(out.set(x, 0, drawn[agent * 2 + 1]));
  return true;
}

const p = new THREE.Vector3();

/** Agent id of the drawn car nearest a screen point within `radius` pixels, or null. */
export function nearestCar(
  drawn: Float32Array,
  scene: THREE.Object3D,
  camera: THREE.Camera,
  clientX: number,
  clientY: number,
  width: number,
  height: number,
  radius = PICK_PX,
): number | null {
  let best: number | null = null;
  let bestSq = radius * radius;
  for (let i = 0; i * 2 < drawn.length; i++) {
    if (!carPosition(drawn, scene, i, p)) continue;
    p.project(camera);
    if (p.z > 1) continue; // behind the camera
    const dx = ((p.x + 1) / 2) * width - clientX;
    const dy = ((1 - p.y) / 2) * height - clientY;
    if (dx * dx + dy * dy < bestSq) [best, bestSq] = [i, dx * dx + dy * dy];
  }
  return best;
}
