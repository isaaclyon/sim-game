// A compact, flat-shaded coal station. Local +z faces the road; the terminal sits behind
// the yard at -z. Visual detail only: the sim still owns the footprint and power output.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { PRESENT_DAY } from './building-kit/style.ts';

const brick = new THREE.Color(0x986750);
const brickDark = new THREE.Color(0x765343);
const concrete = new THREE.Color(0xc3baa7);
const roof = new THREE.Color(0x52616a);
const steel = new THREE.Color(0x748487);
const dark = new THREE.Color(0x354046);
const coal = new THREE.Color(0x303438);
const red = new THREE.Color(0xa84e3d);
const cream = new THREE.Color(0xe6ddc8);
const yellow = new THREE.Color(0xd3a04b);
const glass = new THREE.Color(PRESENT_DAY.glass);
const asphalt = new THREE.Color(0x777973);
const material = new THREE.MeshLambertMaterial({ vertexColors: true });
let geometry: THREE.BufferGeometry | undefined;

/** One shared mesh, not a draw call per window. Snapshot refreshes reuse its GPU buffers. */
export function createCoalPlant(half: number): THREE.Mesh {
  geometry ??= buildGeometry();
  const mesh = new THREE.Mesh(geometry, material);
  mesh.scale.set(half / 22, 1, half / 22);
  mesh.castShadow = mesh.receiveShadow = true;
  return mesh;
}

function buildGeometry(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  // A paved apron separates the plant from the surrounding grass and keeps the entry clear.
  mb.box([0, 0], 0, 44, 0.18, 44, 0, concrete);
  mb.box([0, 0], 0.18, 42.8, 0.04, 42.8, 0, asphalt);
  hall(mb);
  boiler(mb);
  chimney(mb);
  coalYard(mb);
  switchyard(mb);
  // Low perimeter wall with an open front gate aligned to the hall's loading doors.
  for (const x of [-21, 21]) mb.box([x, -1], 0.22, 0.35, 1.2, 40, 0, concrete);
  mb.box([0, -21], 0.22, 42, 1.2, 0.35, 0, concrete);
  for (const [x, width] of [
    [-13, 16],
    [15, 12],
  ])
    mb.box([x, 20.6], 0.22, width, 1.2, 0.35, 0, concrete);
  for (const x of [-4.8, 8.8]) mb.box([x, 20.6], 0.22, 0.65, 2, 0.65, 0, brick);
  // Painted loading apron and short access path; nothing extends beyond the sim footprint.
  for (const x of [-4, 8]) mb.box([x, 17.6], 0.225, 0.15, 0.02, 4.8, 0, cream);
  return mb.geometry();
}

/** Broad brick turbine hall, gabled metal roof and tall factory windows. */
function hall(mb: MeshBuilder) {
  mb.box([-5, 6.5], 0.22, 27, 0.65, 16, 0, concrete);
  mb.box([-5, 6.5], 0.87, 26, 13.5, 15, 0, brick);
  mb.extrude(
    [
      [-18.3, 14.37],
      [8.3, 14.37],
      [-5, 18.2],
    ],
    7.85,
    6.5,
    roof,
  );
  // Long ridge lantern; glass sides catch the light without emissive windows at midday.
  mb.box([-5, 6.5], 17.7, 3, 1.5, 12, 0, steel);
  for (const x of [-6.53, -3.47]) mb.box([x, 6.5], 18, 0.04, 0.8, 11.2, 0, glass);
  mb.box([-5, 6.5], 19.2, 3.7, 0.25, 12.6, 0, roof);
  // Pilasters and cream lintels break up the road-facing elevation.
  for (let x = -17; x <= 7; x += 4) {
    mb.box([x, 14.14], 0.87, 0.5, 13.5, 0.32, 0, brickDark);
  }
  for (const x of [-15, -11, -7, -3, 1, 5]) {
    mb.box([x, 14.06], 6.2, 2.7, 5.6, 0.06, 0, glass);
    mb.box([x, 14.14], 6, 3, 0.25, 0.25, 0, concrete);
    mb.box([x, 14.14], 11.8, 3, 0.25, 0.25, 0, concrete);
    mb.box([x, 14.12], 6.2, 0.12, 5.6, 0.12, 0, cream);
    mb.box([x, 14.12], 8.95, 2.7, 0.12, 0.12, 0, cream);
  }
  for (const z of [1, 5, 9, 13]) {
    mb.box([-18.06, z], 6.2, 0.06, 5.6, 2.4, 0, glass);
    mb.box([-18.15, z], 6, 0.25, 0.25, 2.8, 0, concrete);
    mb.box([-18.12, z], 8.95, 0.12, 0.12, 2.4, 0, cream);
  }
  for (const x of [-1.5, 4.5]) {
    mb.box([x, 14.1], 0.87, 4.5, 4.6, 0.15, 0, dark);
    for (let y = 1.5; y < 5.5; y += 0.7) mb.box([x, 14.2], y, 4.2, 0.06, 0.06, 0, steel);
  }
  mb.box([1.5, 14.8], 5.6, 11.5, 0.22, 2.1, 0, roof);
  // A lower control office gives the entrance a human scale.
  mb.box([14.6, 10.5], 0.22, 10, 5.2, 12, 0, concrete);
  mb.box([14.6, 10.5], 5.42, 10.5, 0.4, 12.5, 0, roof);
  for (const x of [11.7, 14.5]) mb.box([x, 16.56], 2.4, 2, 1.8, 0.06, 0, glass);
  mb.box([17.5, 16.56], 0.22, 1.8, 3.2, 0.06, 0, dark);
  mb.box([17.5, 17.2], 0.22, 2.4, 0.22, 1.3, 0, concrete);
}

/** Taller boiler house and flue leading to the stack, behind the turbine hall. */
function boiler(mb: MeshBuilder) {
  mb.box([-1, -7.3], 0.22, 14, 0.65, 12, 0, concrete);
  mb.box([-1, -7.3], 0.87, 13, 21.5, 11, 0, steel);
  for (const x of [-7.6, 5.6])
    for (const z of [-12.5, -8, -3]) mb.box([x, z], 0.87, 0.25, 21.5, 0.35, 0, dark);
  for (const y of [7, 14, 21.6]) mb.box([-1, -12.9], y, 13.4, 0.25, 0.25, 0, dark);
  mb.box([-1, -7.3], 22.37, 14, 0.4, 12, 0, roof);
  for (const x of [-4, 2]) {
    mb.box([x, -7.3], 22.77, 2.8, 1.3, 4, 0, concrete);
    mb.box([x, -7.3], 24.07, 3.2, 0.15, 4.4, 0, dark);
  }
  mb.box([8.5, -8.5], 10, 7, 3, 3, 0, concrete);
  for (const x of [6.8, 10]) mb.box([x, -8.5], 9.9, 0.3, 3.2, 3.2, 0, dark);
}

/** Tapered, banded chimney with a dark recessed mouth and a service ladder. */
function chimney(mb: MeshBuilder) {
  const c: [number, number] = [13.4, -8.5];
  mb.cylinder(c, 0.22, 4, 1.2, 12, concrete);
  const radius = (y: number) => 2.8 - ((y - 1.42) / 40.58) * 0.85;
  const levels = [1.42, 26, 30, 34, 38, 42];
  const colors = [concrete, red, cream, red, cream];
  for (let i = 0; i < colors.length; i++) {
    const [lo, hi] = [levels[i], levels[i + 1]];
    mb.cylinder(c, lo, radius(lo), hi - lo, 16, colors[i], radius(hi));
  }
  mb.cylinder(c, 42, 2.12, 0.3, 16, dark);
  mb.cylinder(c, 42.31, 1.65, 0.015, 16, coal);
  // Inspection landing and ladder on the outside of the stack, not floating above its taper.
  mb.box([16.3, -8.5], 25.4, 2.1, 0.3, 2.6, 0, dark);
  for (const z of [-9.6, -7.4]) {
    mb.box([17.2, z], 25.7, 0.1, 1.2, 0.1, 0, yellow);
    mb.box([16.3, z], 26.8, 2, 0.1, 0.1, 0, yellow);
  }
  for (const z of [-9, -8]) mb.box([16.6, z], 1.42, 0.1, 24, 0.1, 0, yellow);
  for (let y = 2; y < 25.5; y += 0.65) mb.box([16.6, -8.5], y, 0.12, 0.1, 1, 0, yellow);
}

/** Retaining bunker, faceted coal heaps and a covered inclined conveyor into the boiler. */
function coalYard(mb: MeshBuilder) {
  mb.box([-14.6, -11.5], 0.22, 10, 0.3, 16, 0, dark);
  for (const x of [-19.5, -9.7]) mb.box([x, -11.5], 0.52, 0.35, 2.6, 16, 0, concrete);
  mb.box([-14.6, -19.3], 0.52, 10, 2.6, 0.35, 0, concrete);
  for (const [x, z, radius, height] of [
    [-15.1, -15.1, 3.9, 4.6],
    [-14, -10, 3.6, 5.2],
    [-15.5, -6.4, 3.1, 3.7],
  ])
    mb.cylinder([x, z], 0.52, radius, height, 7, coal, 0.4);
  mb.extrude(
    [
      [-16, 3.8],
      [-5.8, 14],
      [-5.8, 15.3],
      [-16, 5.1],
    ],
    1.1,
    -8,
    roof,
  );
  // Exposed yellow side rails make the diagonal readable at city-view scale.
  for (const z of [-9.15, -6.85])
    mb.extrude(
      [
        [-16, 4.8],
        [-5.8, 15],
        [-5.8, 15.2],
        [-16, 5],
      ],
      0.08,
      z,
      yellow,
    );
  for (const z of [-9, -7]) mb.box([-10.5, z], 0.52, 0.25, 9, 0.25, 0, dark);
}

/** Two transformers and a bus gantry on the terminal side of the plant. */
function switchyard(mb: MeshBuilder) {
  mb.box([4, -17.5], 0.22, 16, 0.3, 5.8, 0, concrete);
  for (const x of [0, 8]) {
    mb.box([x, -17.5], 0.52, 4.2, 3.3, 3, 0, steel);
    for (const side of [-1, 1])
      for (let dx = -1.8; dx <= 1.8; dx += 0.6)
        mb.box([x + dx, -17.5 + side * 1.7], 0.8, 0.16, 2.7, 0.6, 0, dark);
    for (const dx of [-1.2, 0, 1.2]) {
      mb.cylinder([x + dx, -17.5], 3.82, 0.18, 1.5, 6, dark);
      for (const y of [4.1, 4.45, 4.8]) mb.cylinder([x + dx, -17.5], y, 0.3, 0.14, 6, cream);
    }
  }
  for (const x of [-3, 11]) mb.box([x, -19.2], 0.52, 0.3, 7, 0.3, 0, steel);
  mb.box([4, -19.2], 7.52, 14.4, 0.35, 0.35, 0, steel);
  for (const x of [0, 4, 8]) mb.cylinder([x, -19.2], 7.87, 0.22, 0.8, 6, cream);
}
