import type CameraControls from 'camera-controls';
import * as THREE from 'three';

const offset = new THREE.Vector3();
const right = new THREE.Vector3();
const ahead = new THREE.Vector3();
const now = new THREE.Vector3();
const end = new THREE.Vector3();

/**
 * Pans the view along the ground: `toRight` toward the screen's right edge, `forward` toward
 * the top of the screen, both in multiples of the current camera distance, so the speed follows
 * zoom. Height, pitch and zoom stay unchanged.
 *
 * Called every frame at a steady rate, so it moves the current target directly rather than
 * starting a transition: each transition leaves a camera-controls 'rest' listener behind until
 * the camera settles, which piles up thousands during a long pan. Any unfinished mouse pan is
 * shifted by the same step and keeps easing in, instead of being completed in one jump; only
 * that case starts a transition, so listeners stay bounded by the mouse pan's settling time.
 */
export function panAlongGround(controls: CameraControls, toRight: number, forward: number) {
  const camera = controls.camera;
  camera.updateMatrix();
  right.setFromMatrixColumn(camera.matrix, 0);
  ahead.crossVectors(camera.up, right);
  const distance = controls.distance;
  offset
    .copy(right)
    .multiplyScalar(toRight * distance)
    .addScaledVector(ahead, forward * distance);
  controls.getTarget(now, false).add(offset);
  controls.getTarget(end, true).add(offset);
  controls.moveTo(now.x, now.y, now.z, false);
  if (!end.equals(now)) controls.moveTo(end.x, end.y, end.z, true);
}
