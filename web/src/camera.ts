import type CameraControls from 'camera-controls';
import * as THREE from 'three';

const offset = new THREE.Vector3();
const now = new THREE.Vector3();
const end = new THREE.Vector3();
const spherical = new THREE.Spherical();

/**
 * Pans the view along the ground: `toRight` toward the screen's right edge, `forward` toward
 * the top of the screen, both in multiples of the current camera distance, so the speed follows
 * zoom, facing `azimuth` (default: the current view). Height, pitch and zoom stay unchanged.
 *
 * Called every frame at a steady rate, so it moves the current target directly rather than
 * starting a transition: each transition leaves a camera-controls 'rest' listener behind until
 * the camera settles, which piles up thousands during a long pan. Any unfinished mouse pan is
 * shifted by the same step and keeps easing in, instead of being completed in one jump; only
 * that case starts a transition, so listeners stay bounded by the mouse pan's settling time.
 */
export function panAlongGround(
  controls: CameraControls,
  toRight: number,
  forward: number,
  azimuth = controls.azimuthAngle,
) {
  // With y up, screen right on the ground is (cos, 0, -sin) of the azimuth and ahead is
  // (-sin, 0, -cos): the camera sits at +z of its target at azimuth 0, looking down -z.
  const [cos, sin] = [Math.cos(azimuth), Math.sin(azimuth)];
  const distance = controls.distance;
  offset.set(toRight * cos - forward * sin, 0, -toRight * sin - forward * cos);
  offset.multiplyScalar(distance);
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

/** Seconds for a followed car's camera to cover about two thirds of the gap to it. */
export const FOLLOW_EASE = 0.15;

/**
 * Eases the camera's target toward `point` over `seconds`, keeping zoom and angle. It moves the
 * target directly, like `panAlongGround`, so following every frame starts no transitions.
 */
export function followPoint(controls: CameraControls, point: THREE.Vector3, seconds: number) {
  controls.getTarget(now, false);
  now.lerp(point, 1 - Math.exp(-Math.max(seconds, 0) / FOLLOW_EASE));
  controls.moveTo(now.x, now.y, now.z, false);
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
/**
 * Longest integration step. Panning while turning follows an arc; short steps that pan along
 * the mid-step heading keep that arc the same at any frame rate.
 */
const SUBSTEP = 1 / 240;

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
   * solved exactly and arcs are integrated in short steps, so frame rate does not change the
   * path. Returns true if the camera moved.
   */
  update(right: number, forward: number, turn: number, seconds: number): boolean {
    const dt = Math.min(Math.max(seconds, 0), MAX_STEP);
    const pan = Math.max(Math.hypot(right, forward), 1);
    const wanted = [
      (right / pan) * KEY_PAN_SPEED,
      (forward / pan) * KEY_PAN_SPEED,
      Math.sign(turn) * KEY_TURN_SPEED,
    ];
    const steps = Math.ceil(dt / SUBSTEP);
    const h = dt / steps;
    const keep = Math.exp(-h / KEY_EASE);
    const moved = [0, 0, 0];
    let any = false;
    for (let step = 0; step < steps; step++) {
      for (let i = 0; i < 3; i++) {
        const gap = this.speed[i] - wanted[i];
        moved[i] = wanted[i] * h + gap * KEY_EASE * (1 - keep);
        this.speed[i] = wanted[i] + gap * keep;
        // Settle exactly once the glide is too slow to see.
        if (wanted[i] === 0 && Math.abs(this.speed[i]) < 1e-3) this.speed[i] = 0;
      }
      const azimuth = this.controls.azimuthAngle;
      if (moved[0] !== 0 || moved[1] !== 0)
        panAlongGround(this.controls, moved[0], moved[1], azimuth + moved[2] / 2);
      if (moved[2] !== 0) turnAroundTarget(this.controls, moved[2]);
      any ||= moved.some((m) => m !== 0);
    }
    return any;
  }

  /** Stops panning at once and keeps any turn, for a camera that starts following a car. */
  stopPan() {
    this.speed[0] = 0;
    this.speed[1] = 0;
  }

  /** Stops at once, for focus loss and hidden tabs. */
  stop() {
    this.speed.fill(0);
  }
}
