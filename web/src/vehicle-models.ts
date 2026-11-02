// Low-poly vehicle models (issue #29). Display only: a citizen's car model and paint come from
// a hash of its stable agent id, so the same citizen always drives the same car. Every model is
// at most 6 m long, so queues at the sim's 7 m front-to-front spacing never overlap.
//
// Models face +x, stand on y = 0 and are centred on the agent's position. Each has a `body`
// tinted per instance (paint, or speed in traffic view) and `trim` drawn in its own colours.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';

export interface VehicleModel {
  name: string;
  /** Relative share of cars on the road. */
  weight: number;
  length: number;
  body: () => THREE.BufferGeometry;
  trim: () => THREE.BufferGeometry;
}

const WHITE = new THREE.Color(0xffffff);
const GLASS = new THREE.Color(0x2c3640);
const TYRE = new THREE.Color(0x1d1f22);
const HEAD = new THREE.Color(0xf2ecd2);
const TAIL = new THREE.Color(0xb3322f);
const CARGO = new THREE.Color(0xeeeeea);

/** Common paint colours, roughly as often as they appear on real streets. */
export const PAINTS = [
  0xf2f2ee, 0xf2f2ee, 0xb9bec4, 0xb9bec4, 0x6b7077, 0x2a2d31, 0x2a2d31, 0x2f4a78, 0x9e2b2b,
  0x3d6b4f, 0xc9b48a, 0x6f9fc8, 0xd8762f,
].map((c) => new THREE.Color(c));

type Part = (mb: MeshBuilder, color: THREE.Color) => void;

/** A box centred at `x` along the car and `z` across it. */
const box = (x: number, z: number, y0: number, sx: number, sy: number, sz: number): Part => {
  return (mb, color) => mb.box([x, z], y0, sx, sy, sz, 0, color);
};

/** Wheels at `±axle` metres from the centre, `track` wide. */
function wheels(mb: MeshBuilder, axle: number[], track: number, r = 0.34) {
  for (const x of axle)
    for (const z of [-track / 2, track / 2]) mb.box([x, z], 0, r * 2, r * 2, 0.26, 0, TYRE);
}

/** Head and tail lights on a body `length` long and `width` wide, at height `y`. */
function lights(mb: MeshBuilder, length: number, width: number, y: number) {
  for (const z of [-width / 2 + 0.3, width / 2 - 0.3]) {
    mb.box([length / 2, z], y, 0.06, 0.16, 0.34, 0, HEAD);
    mb.box([-length / 2, z], y, 0.06, 0.16, 0.34, 0, TAIL);
  }
}

/** A passenger car: lower body, glass band, roof. `cabin` is `[centre, length]` along x. */
function car(length: number, width: number, bodyH: number, cabin: [number, number], axle: number) {
  const cabinW = width - 0.16;
  const glassY = 0.3 + bodyH;
  return {
    body: () => {
      const mb = new MeshBuilder();
      box(0, 0, 0.3, length, bodyH, width)(mb, WHITE);
      box(cabin[0], 0, glassY + 0.46, cabin[1] - 0.2, 0.1, cabinW)(mb, WHITE);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      box(cabin[0], 0, glassY, cabin[1], 0.46, cabinW)(mb, GLASS);
      wheels(mb, [-axle, axle], width - 0.1);
      lights(mb, length + 0.02, width, glassY - 0.3);
      return mb.geometry();
    },
  };
}

export const VEHICLES: VehicleModel[] = [
  { name: 'sedan', weight: 30, length: 4.6, ...car(4.6, 1.8, 0.62, [-0.15, 2.3], 1.45) },
  { name: 'hatchback', weight: 25, length: 4, ...car(4, 1.75, 0.62, [-0.35, 2.3], 1.3) },
  { name: 'suv', weight: 20, length: 4.7, ...car(4.7, 1.9, 0.8, [-0.35, 2.9], 1.5) },
  {
    name: 'van',
    weight: 10,
    length: 5.2,
    body: () => {
      const mb = new MeshBuilder();
      box(0, 0, 0.32, 5.2, 0.9, 1.95)(mb, WHITE);
      box(-0.45, 0, 1.22, 4.3, 1.05, 1.95)(mb, WHITE);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      box(1.72, 0, 1.3, 0.04, 0.6, 1.8)(mb, GLASS);
      box(1.1, 0, 1.4, 1.2, 0.45, 1.97)(mb, GLASS);
      wheels(mb, [-1.8, 1.7], 1.85, 0.36);
      lights(mb, 5.22, 1.95, 0.85);
      return mb.geometry();
    },
  },
  {
    name: 'pickup',
    weight: 10,
    length: 5.4,
    body: () => {
      const mb = new MeshBuilder();
      box(0, 0, 0.38, 5.4, 0.7, 1.9)(mb, WHITE);
      box(0.55, 0, 1.6, 1.8, 0.1, 1.74)(mb, WHITE);
      // The open bed: two side walls and a tailgate.
      for (const z of [-0.9, 0.9]) box(-1.45, z, 1.08, 2.5, 0.4, 0.1)(mb, WHITE);
      box(-2.65, 0, 1.08, 0.1, 0.4, 1.9)(mb, WHITE);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      box(0.55, 0, 1.08, 2, 0.52, 1.74)(mb, GLASS);
      wheels(mb, [-1.8, 1.7], 1.8, 0.38);
      lights(mb, 5.42, 1.9, 0.8);
      return mb.geometry();
    },
  },
  {
    name: 'box van',
    weight: 5,
    length: 6,
    // Only the cab is painted; the cargo box stays white, like a rental or delivery van.
    body: () => {
      const mb = new MeshBuilder();
      box(2.2, 0, 0.4, 1.6, 1.0, 2)(mb, WHITE);
      box(2.05, 0, 1.4, 1.3, 0.12, 1.9)(mb, WHITE);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      box(2.12, 0, 1.4, 1.4, 0.5, 1.92)(mb, GLASS);
      box(-0.8, 0, 0.5, 4.4, 2.5, 2.2)(mb, CARGO);
      box(0, 0, 0.38, 5.8, 0.14, 1.8)(mb, TYRE);
      wheels(mb, [-2, 2.2], 1.95, 0.4);
      lights(mb, 6.02, 2, 0.75);
      return mb.geometry();
    },
  },
];

/** Integer hash of an agent id to [0, 1); `salt` picks an independent choice. */
export function agentHash(agent: number, salt: number): number {
  let n = Math.imul(agent ^ (salt * 0x9e3779b9), 0x85ebca6b);
  n ^= n >>> 13;
  n = Math.imul(n, 0xc2b2ae35);
  n ^= n >>> 16;
  return (n >>> 0) / 4294967296;
}

const TOTAL = VEHICLES.reduce((sum, v) => sum + v.weight, 0);

/** The model index a citizen's car uses; stable for the agent id. */
export function vehicleModel(agent: number): number {
  let r = agentHash(agent, 1) * TOTAL;
  for (let i = 0; i < VEHICLES.length; i++) {
    r -= VEHICLES[i].weight;
    if (r < 0) return i;
  }
  return 0;
}

export function vehiclePaint(agent: number): THREE.Color {
  return PAINTS[Math.floor(agentHash(agent, 2) * PAINTS.length)];
}
