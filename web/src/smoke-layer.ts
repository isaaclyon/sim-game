// Smoke from factory chimneys: low-poly puffs that leave the stack top, grow, drift downwind and
// shrink away. Display only, like the haze (ADR 0037): no sim state, timed by the clock alone.
// Each puff's place is a pure function of time, so there is nothing to step or save.
import * as THREE from 'three';
import { smokestack } from './building-kit/kit.ts';
import { RISE_SECONDS } from './building-kit/layer.ts';
import type { Pt } from './building-kit/polygon.ts';
import { SMOKE } from './palette.ts';

/** Seconds from leaving the stack to fading out. */
export const PUFF_LIFE = 8;
const RISE = 1.8;
/** Display-only breeze, metres a second in scene-local (x, z). */
const WIND: Pt = [1.3, 0.55];
/** Chimneys drawn at once; the ones nearest the camera target win. */
export const MAX_STACKS = 300;
const RESORT_METRES = 32;

export interface Stack {
  x: number;
  z: number;
  top: number;
  /** Puff size, larger for bigger factories. */
  size: number;
  /** Puffs alive at once, more for high-density industry. */
  puffs: number;
  /** Seconds offset, so neighbouring chimneys do not puff in step. */
  phase: number;
  /** Clock time the chimney starts smoking: after a new factory has risen. */
  from: number;
}

/** Position and scale of puff `k` of `stack` at `now` seconds; scale 0 when not showing. */
export function puffAt(stack: Stack, k: number, now: number): [number, number, number, number] {
  const t = now - stack.from;
  const age =
    (((t + stack.phase + (k * PUFF_LIFE) / stack.puffs) % PUFF_LIFE) + PUFF_LIFE) % PUFF_LIFE;
  // Before the chimney starts, or a puff that would have left before then.
  if (t < 0 || age > t) return [stack.x, stack.top, stack.z, 0];
  const u = age / PUFF_LIFE;
  const drift = age ** 1.3;
  const wobble = Math.sin(age * 1.7 + k * 2.1) * 0.35;
  const envelope =
    THREE.MathUtils.smoothstep(u, 0, 0.08) * (1 - THREE.MathUtils.smoothstep(u, 0.6, 1));
  return [
    stack.x + WIND[0] * drift * 0.6 - WIND[1] * wobble,
    stack.top + RISE * age,
    stack.z + WIND[1] * drift * 0.6 + WIND[0] * wobble,
    stack.size * (0.8 + 2.2 * u) * envelope,
  ];
}

function area(ring: Pt[]): number {
  let a = 0;
  for (let i = 0; i < ring.length; i++) {
    const [p, q] = [ring[i], ring[(i + 1) % ring.length]];
    a += p[0] * q[1] - q[0] * p[1];
  }
  return Math.abs(a) / 2;
}

export class SmokeLayer {
  readonly group = new THREE.Group();
  private stacks = new Map<number, Stack>();
  private shown: Stack[] = [];
  private sortedAt: Pt | null = null;
  private mesh?: THREE.InstancedMesh;
  private readonly matrix = new THREE.Matrix4();
  private readonly position = new THREE.Vector3();
  private readonly scale = new THREE.Vector3();
  private readonly turn = new THREE.Quaternion();
  private readonly glow = new THREE.Color(SMOKE.glow);
  private readonly night: { value: number };

  /** `night` is the lighting's 0 (day) to 1 (night) share; the smoke's own glow fades by it. */
  constructor(night: { value: number } = { value: 0 }) {
    this.night = night;
  }

  /**
   * Takes the whole building snapshot (see `buildings` in protocol.ts). With `now` (seconds),
   * chimneys of buildings new since the last snapshot start smoking once they have risen.
   */
  update(data: Float32Array, ids: Uint32Array, now: number | null = null) {
    const next = new Map<number, Stack>();
    for (let o = 0, i = 0; o < data.length; o += 5 + 2 * data[o + 4], i++) {
      const [zone, height, roof, front, n] = data.subarray(o, o + 5);
      if (zone < 5) continue;
      const known = this.stacks.get(ids[i]);
      if (known) {
        next.set(ids[i], known);
        continue;
      }
      const ring: Pt[] = [];
      for (let k = 0; k < n; k++) ring.push([data[o + 5 + 2 * k], data[o + 6 + 2 * k]]);
      const stack = smokestack({ zone, height, roof, front, ring });
      if (!stack) continue;
      const size = THREE.MathUtils.clamp(Math.sqrt(area(ring)) / 20, 0.8, 1.6);
      next.set(ids[i], {
        x: stack.at[0],
        z: stack.at[1],
        top: stack.top,
        size,
        puffs: zone === 6 ? 16 : 11,
        phase: (ids[i] * 2.39996) % PUFF_LIFE,
        from: now === null ? 0 : now + RISE_SECONDS,
      });
    }
    this.stacks = next;
    this.sortedAt = null;
  }

  /** Moves every puff to `now` (seconds), drawing the chimneys nearest `focus` (local x, z). */
  animate(now: number, focus: Pt) {
    const moved =
      !this.sortedAt ||
      Math.hypot(focus[0] - this.sortedAt[0], focus[1] - this.sortedAt[1]) > RESORT_METRES;
    if (moved) {
      const all = [...this.stacks.values()];
      const d = (s: Stack) => (s.x - focus[0]) ** 2 + (s.z - focus[1]) ** 2;
      if (all.length > MAX_STACKS) all.sort((a, b) => d(a) - d(b));
      this.shown = all.slice(0, MAX_STACKS);
      this.sortedAt = focus;
    }
    const count = this.shown.reduce((sum, s) => sum + s.puffs, 0);
    const mesh = this.meshFor(count);
    let i = 0;
    for (const stack of this.shown)
      for (let k = 0; k < stack.puffs; k++, i++) {
        const [x, y, z, s] = puffAt(stack, k, now);
        // A fixed turn per puff, so the facets do not line up.
        this.turn.setFromAxisAngle(this.position.set(0, 1, 0), k * 1.3 + stack.phase);
        this.matrix.compose(this.position.set(x, y, z), this.turn, this.scale.set(s, s * 0.85, s));
        mesh.setMatrixAt(i, this.matrix);
      }
    mesh.count = i;
    mesh.instanceMatrix.needsUpdate = true;
    (mesh.material as THREE.MeshLambertMaterial).emissive
      .copy(this.glow)
      .multiplyScalar(1 - this.night.value);
  }

  private meshFor(count: number): THREE.InstancedMesh {
    if (this.mesh && this.mesh.instanceMatrix.count >= count) return this.mesh;
    if (this.mesh) {
      this.group.remove(this.mesh);
      this.mesh.dispose();
    }
    const capacity = Math.max(64, 2 ** Math.ceil(Math.log2(count)));
    const material = new THREE.MeshLambertMaterial({
      color: SMOKE.puff,
      // Lit from within a little, so shading reads as soft cloud rather than stone.
      emissive: SMOKE.glow,
      flatShading: true,
      transparent: true,
      opacity: SMOKE.opacity,
      depthWrite: false,
    });
    this.mesh = new THREE.InstancedMesh(new THREE.IcosahedronGeometry(1, 1), material, capacity);
    this.mesh.frustumCulled = false;
    this.mesh.count = 0;
    this.group.add(this.mesh);
    return this.mesh;
  }
}
