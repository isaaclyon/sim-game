// Low-poly walker figures with a simple walk cycle (issue #29). Display only: clothes and skin
// come from a hash of the stable agent id; the stride comes from how far the walker has moved,
// so figures stand still while waiting at signals and while the game is paused.
//
// Parts face +x and stand on y = 0. Legs and arms hang from a pivot at their origin, so one
// rotation about z swings them forward and back.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { agentHash } from './vehicle-models.ts';

const HIP = 0.9;
const SHOULDER = 1.42;
/** Metres travelled per full stride cycle, both legs. */
const STRIDE = 1.5;
/** Peak leg swing, radians; arms swing a little less. */
const SWING = 0.5;

const WHITE = new THREE.Color(0xffffff);

export const SHIRTS = [
  0x3f6fb5, 0xc8423b, 0xe3b23c, 0x4f8f5b, 0xeeeeea, 0x2f3338, 0x8a5aa8, 0xd97a9a, 0x5aa3a8,
].map((c) => new THREE.Color(c));
export const TROUSERS = [0x2f3a52, 0x3a3a3e, 0x6b5a45, 0x24272b, 0x5d6f86].map(
  (c) => new THREE.Color(c),
);
export const SKIN = [0xf1d0b5, 0xe0b48f, 0xc68e64, 0x8d5a3b, 0x5e3a26].map(
  (c) => new THREE.Color(c),
);

const pick = (list: THREE.Color[], agent: number, salt: number) =>
  list[Math.floor(agentHash(agent, salt) * list.length)];

export const shirtOf = (agent: number) => pick(SHIRTS, agent, 3);
export const trousersOf = (agent: number) => pick(TROUSERS, agent, 4);
export const skinOf = (agent: number) => pick(SKIN, agent, 5);

function part(build: (mb: MeshBuilder) => void): () => THREE.BufferGeometry {
  return () => {
    const mb = new MeshBuilder();
    build(mb);
    return mb.geometry();
  };
}

/** Geometry per part, all white so the instance colour is the part's colour. */
export const FIGURE = {
  torso: part((mb) => mb.box([0, 0], HIP, 0.24, SHOULDER - HIP + 0.06, 0.42, 0, WHITE)),
  head: part((mb) => {
    mb.box([0, 0], SHOULDER + 0.06, 0.08, 0.08, 0.08, 0, WHITE);
    mb.box([0, 0], SHOULDER + 0.12, 0.22, 0.24, 0.2, 0, WHITE);
  }),
  leg: part((mb) => mb.box([0, 0], -HIP, 0.16, HIP, 0.15, 0, WHITE)),
  arm: part((mb) => mb.box([0, 0], -0.6, 0.11, 0.6, 0.11, 0, WHITE)),
};

/** Where each limb pivots, across the body (z); legs at the hip, arms at the shoulder. */
export const LIMBS = {
  leg: { y: HIP, z: [-0.1, 0.1] },
  arm: { y: SHOULDER, z: [-0.27, 0.27] },
};

/**
 * Swing angles `[leg, arm]` for the left side; the right side is the negation. `walked` is
 * metres along the walker's heading, `moving` is 0 when standing and 1 at full pace.
 */
export function swing(walked: number, moving: number, agent: number): [number, number] {
  const phase = (walked / STRIDE + agentHash(agent, 6)) * Math.PI * 2;
  const leg = Math.sin(phase) * SWING * Math.min(Math.max(moving, 0), 1);
  return [leg, -leg * 0.8];
}
