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
  /** A painted two-box stand-in for cars far from the camera. */
  far: () => THREE.BufferGeometry;
}

const WHITE = new THREE.Color(0xffffff);
const GLASS = new THREE.Color(0x2c3640);
const TYRE = new THREE.Color(0x1d1f22);
const HEAD = new THREE.Color(0xf2ecd2);
const TAIL = new THREE.Color(0xb3322f);
const CARGO = new THREE.Color(0xeeeeea);
const HUB = new THREE.Color(0x9a9ea3);
/** The van's upper body: a raked windscreen over a short hood, then a flat roof. */
const VAN_UPPER: [number, number][] = [
  [-2.6, 1.22],
  [1.95, 1.22],
  [1.35, 2.27],
  [-2.55, 2.27],
  [-2.6, 2.2],
];

/** Common paint colours, roughly as often as they appear on real streets. */
export const PAINTS = [
  0xf2f2ee, 0xf2f2ee, 0xb9bec4, 0xb9bec4, 0x6b7077, 0x2a2d31, 0x2a2d31, 0x2f4a78, 0x9e2b2b,
  0x3d6b4f, 0xc9b48a, 0x6f9fc8, 0xd8762f,
].map((c) => new THREE.Color(c));

type Part = (mb: MeshBuilder, color: THREE.Color) => void;
type Pt = [number, number];

/** A box centred at `x` along the car and `z` across it. */
const box = (x: number, z: number, y0: number, sx: number, sy: number, sz: number): Part => {
  return (mb, color) => mb.box([x, z], y0, sx, sy, sz, 0, color);
};

/** A ring of `n` points of radius `r` around `c` in the side view (x along the car, y up). */
const ring = (c: Pt, r: number, n: number): Pt[] =>
  Array.from({ length: n }, (_, i) => {
    const a = ((i + 0.5) / n) * Math.PI * 2;
    return [c[0] + Math.cos(a) * r, c[1] + Math.sin(a) * r];
  });

/** Eight-sided tyres with a grey hub, at `±axle` metres from the centre, `track` wide. */
function wheels(mb: MeshBuilder, axle: number[], track: number, r = 0.34) {
  // The octagon's flat bottom edge, not a corner, rests on the road.
  const y = r * Math.cos(Math.PI / 8);
  for (const x of axle)
    for (const s of [-1, 1]) {
      mb.extrude(ring([x, y], r, 8), 0.13, (s * track) / 2, TYRE);
      // The hub is one flat face just outside the tyre; nobody sees its back.
      const z = s * (track / 2 + 0.135);
      const hub = ring([x, y], r * 0.5, 6);
      for (let i = 0; i < 6; i++) {
        const [p, q] = [hub[i], hub[(i + 1) % 6]];
        mb.tri([x, y, z], [p[0], p[1], z], [q[0], q[1], z], HUB, [0, 0, s]);
      }
    }
}

/** Head and tail lights on a body `length` long and `width` wide, at height `y`. */
function lights(mb: MeshBuilder, length: number, width: number, y: number) {
  for (const z of [-width / 2 + 0.3, width / 2 - 0.3]) {
    mb.box([length / 2, z], y, 0.06, 0.14, 0.34, 0, HEAD);
    mb.box([-length / 2, z], y, 0.06, 0.14, 0.34, 0, TAIL);
  }
}

/**
 * The lower body from `x0` (rear) to `x1` (front), `base` to `top` high: rounded bumpers and a
 * hood that rises to the windscreen at `cowl`. The upper edges are bevelled inwards.
 */
function lowerBody(mb: MeshBuilder, x0: number, x1: number, w: number, base: number, top: number) {
  const shape: Pt[] = [
    [x0 + 0.1, base],
    [x1 - 0.15, base],
    [x1, base + 0.12],
    [x1, top - 0.18],
    [x1 - 0.18, top - 0.08],
    [x1 - 0.6, top],
    [x0 + 0.1, top],
    [x0, top - 0.1],
    [x0, base + 0.1],
  ];
  mb.extrude(shape, bevel(w / 2, top), 0, WHITE);
}

/** Half-width `hw` that narrows by 7 cm over the last 20 cm below `top`. */
const bevel = (hw: number, top: number) => (y: number) =>
  hw - 0.07 * Math.min(Math.max((y - top + 0.2) / 0.2, 0), 1);

/**
 * A glasshouse from `x0` to `x1` on top of the body at `y`, `h` tall: the windscreen leans back
 * by `front` metres, the rear window by `rear`, and the sides lean in. Glass goes to `glass`,
 * the painted roof to `roof`.
 */
function cabin(
  glass: MeshBuilder | null,
  roof: MeshBuilder | null,
  x: [number, number],
  w: number,
  y: number,
  h: number,
  [front, rear]: [number, number],
) {
  const [x0, x1] = x;
  const top = y + h;
  glass?.extrude(
    [
      [x0, y - 0.02],
      [x1, y - 0.02],
      [x1 - front, top],
      [x0 + rear, top],
    ],
    (yy) => w / 2 - (0.1 * (yy - y)) / h,
    0,
    GLASS,
  );
  roof?.extrude(
    [
      [x0 + rear - 0.03, top],
      [x1 - front + 0.03, top],
      [x1 - front - 0.04, top + 0.1],
      [x0 + rear + 0.06, top + 0.1],
    ],
    bevel(w / 2 - 0.08, top + 0.1),
    0,
    WHITE,
  );
}

/** Glass laid flat against the body edge from `a` to `b`, `hw` either side of the centre. */
function pane(mb: MeshBuilder, a: Pt, b: Pt, hw: number) {
  const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
  const n: Pt = [((b[1] - a[1]) / len) * 0.03, (-(b[0] - a[0]) / len) * 0.03];
  mb.extrude([a, b, [b[0] + n[0], b[1] + n[1]], [a[0] + n[0], a[1] + n[1]]], hw, 0, GLASS);
}

/** Darkens the paint on the far model's upper box, so it still reads as glass and roof. */
const FAR_CABIN = new THREE.Color(0x5a5a5a);

/**
 * The far model: a body `length` by `width` up to `top`, and a darker box over `x = [x0, x1]` up
 * to `roof`. Painted like the body, about 24 triangles.
 */
function far(length: number, width: number, top: number, roof: number, [x0, x1]: number[]) {
  return () => {
    const mb = new MeshBuilder();
    mb.box([0, 0], 0.15, length, top - 0.15, width, 0, WHITE);
    mb.box([(x0 + x1) / 2, 0], top, x1 - x0, roof - top, width - 0.2, 0, FAR_CABIN);
    return mb.geometry();
  };
}

/**
 * A passenger car `length` by `width`: a body `bodyH` tall above the road, then a glasshouse over
 * `cabin = [centre, length]` whose windscreen and rear window lean by `rake = [front, rear]`.
 */
function car(
  length: number,
  width: number,
  bodyH: number,
  [c, len]: [number, number],
  axle: number,
  rake: [number, number],
) {
  const top = 0.3 + bodyH;
  const x: [number, number] = [c - len / 2, c + len / 2];
  return {
    body: () => {
      const mb = new MeshBuilder();
      lowerBody(mb, -length / 2, length / 2, width, 0.3, top);
      cabin(null, mb, x, width - 0.16, top, 0.46, rake);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      cabin(mb, null, x, width - 0.16, top, 0.46, rake);
      wheels(mb, [-axle, axle], width - 0.1);
      lights(mb, length + 0.02, width, top - 0.32);
      return mb.geometry();
    },
    far: far(length, width, top, top + 0.56, x),
  };
}

export const VEHICLES: VehicleModel[] = [
  {
    name: 'sedan',
    weight: 30,
    length: 4.6,
    ...car(4.6, 1.8, 0.62, [-0.15, 2.4], 1.45, [0.6, 0.5]),
  },
  {
    name: 'hatchback',
    weight: 25,
    length: 4,
    ...car(4, 1.75, 0.62, [-0.35, 2.3], 1.3, [0.6, 0.2]),
  },
  { name: 'suv', weight: 20, length: 4.7, ...car(4.7, 1.9, 0.8, [-0.35, 2.9], 1.5, [0.5, 0.12]) },
  {
    name: 'van',
    weight: 10,
    length: 5.2,
    body: () => {
      const mb = new MeshBuilder();
      lowerBody(mb, -2.6, 2.6, 1.95, 0.32, 1.22);
      mb.extrude(VAN_UPPER, bevel(0.95, 2.27), 0, WHITE);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      pane(mb, [1.9, 1.3], [1.45, 2.1], 0.82);
      box(0.5, 0, 1.5, 1.8, 0.42, 1.92)(mb, GLASS);
      wheels(mb, [-1.8, 1.7], 1.85, 0.36);
      lights(mb, 5.22, 1.95, 0.85);
      return mb.geometry();
    },
    far: far(5.2, 1.95, 1.22, 2.27, [-2.6, 1.65]),
  },
  {
    name: 'pickup',
    weight: 10,
    length: 5.4,
    body: () => {
      const mb = new MeshBuilder();
      lowerBody(mb, -2.7, 2.7, 1.9, 0.38, 1.08);
      cabin(null, mb, [-0.45, 1.55], 1.74, 1.08, 0.52, [0.45, 0.08]);
      // The open bed: two side walls and a tailgate.
      for (const z of [-0.83, 0.83]) box(-1.45, z, 1.08, 2.5, 0.4, 0.1)(mb, WHITE);
      box(-2.65, 0, 1.08, 0.1, 0.4, 1.76)(mb, WHITE);
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      cabin(mb, null, [-0.45, 1.55], 1.74, 1.08, 0.52, [0.45, 0.08]);
      wheels(mb, [-1.8, 1.7], 1.8, 0.38);
      lights(mb, 5.42, 1.9, 0.76);
      return mb.geometry();
    },
    far: far(5.4, 1.9, 1.08, 1.7, [-0.45, 1.55]),
  },
  {
    name: 'box van',
    weight: 5,
    length: 6,
    // Only the cab is painted; the cargo box stays white, like a rental or delivery van.
    body: () => {
      const mb = new MeshBuilder();
      lowerBody(mb, 1.4, 3, 2, 0.4, 1.35);
      mb.extrude(
        [
          [1.4, 1.35],
          [2.4, 1.35],
          [2.1, 2.02],
          [1.4, 2.02],
        ],
        bevel(0.95, 2.02),
        0,
        WHITE,
      );
      return mb.geometry();
    },
    trim: () => {
      const mb = new MeshBuilder();
      pane(mb, [2.37, 1.42], [2.12, 1.97], 0.84);
      box(1.78, 0, 1.45, 0.5, 0.4, 1.92)(mb, GLASS);
      box(-0.8, 0, 0.5, 4.4, 2.5, 2.2)(mb, CARGO);
      box(0, 0, 0.38, 5.8, 0.14, 1.8)(mb, TYRE);
      wheels(mb, [-2, 2.2], 1.9, 0.4);
      lights(mb, 6.02, 2, 0.8);
      return mb.geometry();
    },
    // The cargo box stands in for the cabin, so it is drawn as tall as the real one.
    far: far(6, 2, 1.35, 3, [-3, 1.4]),
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
