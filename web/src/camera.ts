import type CameraControls from 'camera-controls';

/**
 * Pans the view along the ground: `right` toward the screen's right edge, `forward` toward the
 * top of the screen, both in multiples of the current camera distance, so the speed follows
 * zoom. Height, pitch and zoom stay unchanged; the camera's damping smooths the motion.
 */
export function panAlongGround(controls: CameraControls, right: number, forward: number) {
  const distance = controls.distance;
  // truck refreshes the camera matrix that forward reads, so it goes first.
  controls.truck(right * distance, 0, true);
  controls.forward(forward * distance, true);
}
