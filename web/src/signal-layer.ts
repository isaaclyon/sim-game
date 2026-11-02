// Traffic signals as instanced low-poly mast arms, one per signalised approach (display only).
// The sim gives `[x, y, heading, state]` at the stop line, 6 m right of the road centreline.
// A pole stands there; its arm reaches 4 m left over the approach lane, and the head hangs
// there facing oncoming drivers. Only the lamps the state calls for glow.
//
// Models face the direction of travel (+x); +z is the driver's left; they stand on y = 0.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { ensureInstances } from './instances.ts';

const SIGNAL_STRIDE = 4;
const STEEL = new THREE.Color(0x8d9196);
const HOUSING = new THREE.Color(0x2b2e33);
const BACKPLATE = new THREE.Color(0x1d1f23);
const WHITE = new THREE.Color(0xffffff);

/** Where the head hangs, metres left of the pole: over the lane centre. */
const HEAD_Z = 4;
const ARROW_Z = 4.55;
const LAMP = 0.32;

type Lamp = 'red' | 'amber' | 'green' | 'arrow';

function geometry(build: (mb: MeshBuilder) => void): () => THREE.BufferGeometry {
  return () => {
    const mb = new MeshBuilder();
    build(mb);
    return mb.geometry();
  };
}

/** Lamp centres `[z, y]`, top to bottom, then the turn arrow beside the green lamp. */
const AT: Record<Lamp, [number, number]> = {
  red: [HEAD_Z, 5.32],
  amber: [HEAD_Z, 4.88],
  green: [HEAD_Z, 4.44],
  arrow: [ARROW_Z, 4.44],
};

/** Pole, arm, housings and visors: everything that does not light up. */
export const POLE = geometry((mb) => {
  mb.box([0, 0], 0, 0.45, 0.25, 0.45, 0, STEEL);
  mb.box([0, 0], 0, 0.22, 6.3, 0.22, 0, STEEL);
  mb.box([0, 2.4], 5.95, 0.16, 0.16, 4.8, 0, STEEL);
  mb.box([0, HEAD_Z], 5.55, 0.06, 0.45, 0.06, 0, STEEL);
  mb.box([0, HEAD_Z], 4.15, 0.4, 1.45, 0.48, 0, HOUSING);
  mb.box([0.22, HEAD_Z], 4.05, 0.04, 1.65, 0.72, 0, BACKPLATE);
  mb.box([0, ARROW_Z], 4.2, 0.4, 0.48, 0.44, 0, HOUSING);
  for (const [z, y] of Object.values(AT)) mb.box([-0.3, z], y + 0.14, 0.2, 0.04, 0.36, 0, HOUSING);
});

const lamp = (lit: number, dim: number) => ({
  lit: new THREE.Color(lit),
  dim: new THREE.Color(dim),
});
const SHADES: Record<Lamp, { lit: THREE.Color; dim: THREE.Color }> = {
  red: lamp(0xff3b30, 0x3f1a19),
  amber: lamp(0xffb020, 0x3d3019),
  green: lamp(0x35e07a, 0x173826),
  arrow: lamp(0x35e07a, 0x173826),
};

/** Each lamp's face, white so the instance colour decides lit or dim. */
export const LAMPS = Object.fromEntries(
  (Object.keys(AT) as Lamp[]).map((k) => {
    const [z, y] = AT[k];
    const face = geometry((mb) => mb.box([-0.22, z], y - LAMP / 2, 0.04, LAMP, LAMP, 0, WHITE));
    return [k, { geometry: face, ...SHADES[k] }];
  }),
) as Record<Lamp, { geometry: () => THREE.BufferGeometry; lit: THREE.Color; dim: THREE.Color }>;

/** Lamps lit for a signal state: 0 red, 1 green, 2 green plus the protected-left arrow. */
export function litLamps(state: number): Lamp[] {
  if (state === 1) return ['green'];
  if (state === 2) return ['green', 'arrow'];
  return ['red'];
}

export class SignalLayer {
  private poles?: THREE.InstancedMesh;
  private readonly lamps: Partial<Record<Lamp, THREE.InstancedMesh>> = {};
  private readonly poleMaterial = new THREE.MeshLambertMaterial({
    vertexColors: true,
    flatShading: true,
  });
  /** Unlit, so a glowing lamp keeps its colour on the shaded side of a street. */
  private readonly lampMaterial = new THREE.MeshBasicMaterial();
  private readonly matrix = new THREE.Matrix4();
  private readonly quat = new THREE.Quaternion();
  private readonly pos = new THREE.Vector3();
  private readonly scale = new THREE.Vector3(1, 1, 1);
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly scene: THREE.Object3D;

  constructor(scene: THREE.Object3D) {
    this.scene = scene;
  }

  /** `signals` is `[x, y, heading, state]` per signalised approach, at its stop line. */
  draw(signals: Float32Array) {
    const count = signals.length / SIGNAL_STRIDE;
    this.poles = ensureInstances(this.scene, this.poles, count, POLE, this.poleMaterial);
    const kinds = Object.keys(LAMPS) as Lamp[];
    for (const k of kinds) {
      this.lamps[k] = ensureInstances(
        this.scene,
        this.lamps[k],
        count,
        LAMPS[k].geometry,
        this.lampMaterial,
      );
    }
    for (let i = 0; i < count; i++) {
      const o = i * SIGNAL_STRIDE;
      this.pos.set(signals[o], 0, signals[o + 1]);
      // Local +z holds sim +y, so negate here; the scene reflects all positions and headings.
      this.quat.setFromAxisAngle(this.up, -signals[o + 2]);
      this.matrix.compose(this.pos, this.quat, this.scale);
      this.poles.setMatrixAt(i, this.matrix);
      const lit = litLamps(signals[o + 3]);
      for (const k of kinds) {
        const mesh = this.lamps[k] as THREE.InstancedMesh;
        mesh.setMatrixAt(i, this.matrix);
        mesh.setColorAt(i, lit.includes(k) ? LAMPS[k].lit : LAMPS[k].dim);
      }
    }
    for (const mesh of [this.poles, ...kinds.map((k) => this.lamps[k])]) {
      if (!mesh) continue;
      mesh.count = count;
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
  }
}
