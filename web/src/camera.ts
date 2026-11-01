import type CameraControls from 'camera-controls';

/**
 * Pans the view along the ground: `right` toward the screen's right edge, `forward` toward the
 * top of the screen, both in multiples of the current camera distance, so the speed follows
 * zoom. Height, pitch and zoom stay unchanged. Called every frame at a steady rate, so it
 * moves without transitions: each animated move would leave a camera-controls 'rest' listener
 * behind until the camera settles, piling up thousands during a long pan.
 */
export function panAlongGround(controls: CameraControls, right: number, forward: number) {
  const distance = controls.distance;
  // truck refreshes the camera matrix that forward reads, so it goes first.
  controls.truck(right * distance, 0, false);
  controls.forward(forward * distance, false);
}
