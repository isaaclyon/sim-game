import type CameraControls from 'camera-controls';
import * as THREE from 'three';

const offset = new THREE.Vector3();
const right = new THREE.Vector3();
const ahead = new THREE.Vector3();
const now = new THREE.Vector3();
const end = new THREE.Vector3();
const spherical = new THREE.Spherical();

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

/**
 * Turns the view around the point it looks at by `angle` radians (positive turns the view to
 * the left, as Q does). Like `panAlongGround`, it moves the current angle directly and keeps
 * any unfinished mouse orbit easing in, starting a transition only while one is settling.
 */
export function turnAroundTarget(controls: CameraControls, angle: number) {
  const now = controls.azimuthAngle;
  controls.getSpherical(spherical, true);
  controls.azimuthAngle = now + angle;
  if (spherical.theta !== now) controls.rotateTo(spherical.theta + angle, spherical.phi, true);
}

/** Top keyboard pan speed in camera distances per second, so it looks the same at any zoom. */
export const KEY_PAN_SPEED = 1;
/** Top keyboard turn speed in radians per second: a quarter turn. */
export const KEY_TURN_SPEED = Math.PI / 2;
/**
 * Seconds for keyboard motion to cover about two thirds of the change to a new speed. Speed
 * eases toward the keys' speed on this curve, so motion builds up when a key goes down and
 * glides to a stop when it comes up.
 */
export const KEY_EASE = 0.2;
/** Longest frame step, so a stalled or background tab cannot jump the camera. */
const MAX_STEP = 0.1;

/** Keyboard camera motion with eased speed: pan along the ground and turn around the target. */
export class KeyCamera {
  /** Current speeds: right and forward pan, then turn. */
  private readonly speed = [0, 0, 0];

  private readonly controls: CameraControls;

  constructor(controls: CameraControls) {
    this.controls = controls;
  }

  /**
   * Advances `seconds` with the keys asking for `right`, `forward` and `turn`, each -1 to 1
   * (positive turn is to the left). Diagonal pans are no faster than straight ones. The ease is
   * solved exactly, so frame rate does not change the path. Returns true if the camera moved.
   */
  update(right: number, forward: number, turn: number, seconds: number): boolean {
    const dt = Math.min(Math.max(seconds, 0), MAX_STEP);
    const pan = Math.max(Math.hypot(right, forward), 1);
    const wanted = [
      (right / pan) * KEY_PAN_SPEED,
      (forward / pan) * KEY_PAN_SPEED,
      Math.sign(turn) * KEY_TURN_SPEED,
    ];
    const keep = Math.exp(-dt / KEY_EASE);
    const moved = [0, 0, 0];
    for (let i = 0; i < 3; i++) {
      const gap = this.speed[i] - wanted[i];
      moved[i] = wanted[i] * dt + gap * KEY_EASE * (1 - keep);
      this.speed[i] = wanted[i] + gap * keep;
      // Settle exactly once the glide is too slow to see.
      if (wanted[i] === 0 && Math.abs(this.speed[i]) < 1e-3) this.speed[i] = 0;
    }
    if (moved[0] !== 0 || moved[1] !== 0) panAlongGround(this.controls, moved[0], moved[1]);
    if (moved[2] !== 0) turnAroundTarget(this.controls, moved[2]);
    return moved.some((m) => m !== 0);
  }

  /** Stops at once, for focus loss and hidden tabs. */
  stop() {
    this.speed.fill(0);
  }
}
