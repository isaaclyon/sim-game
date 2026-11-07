// Low-poly pump house and water tower (ADR 0052), in the coal plant's flat-shaded style.
// Local +z faces the road. Visual detail only: the sim owns footprints, output and levels.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { WaterKind } from './protocol.ts';

const concrete = new THREE.Color(0xc3baa7);
const brick = new THREE.Color(0x9a6a52);
const roof = new THREE.Color(0x52616a);
const pipe = new THREE.Color(0x3f7fa6);
const steel = new THREE.Color(0x748487);
const tank = new THREE.Color(0x8fb8cf);
const tankDark = new THREE.Color(0x5f8ea8);
const cream = new THREE.Color(0xe6ddc8);
const door = new THREE.Color(0x354046);
const material = new THREE.MeshLambertMaterial({ vertexColors: true });
/** The half size each model is drawn at; the sim's footprint scales it. */
export const MODEL_HALF: Record<WaterKind, number> = { [WaterKind.Pump]: 7, [WaterKind.Tower]: 5 };
const geometries = new Map<WaterKind, THREE.BufferGeometry>();

/** One shared mesh per kind, scaled to the footprint's half size. */
export function createWaterPiece(kind: WaterKind, half: number): THREE.Mesh {
  let geometry = geometries.get(kind);
  if (!geometry) {
    geometry = kind === WaterKind.Pump ? pump() : tower();
    geometries.set(kind, geometry);
  }
  const mesh = new THREE.Mesh(geometry, material);
  const s = half / MODEL_HALF[kind];
  mesh.scale.set(s, 1, s);
  mesh.castShadow = mesh.receiveShadow = true;
  return mesh;
}

/** A brick pump house on a concrete pad, a wellhead behind it and a blue main to the road. */
function pump(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 14, 0.18, 14, 0, concrete);
  mb.box([0, -1], 0.18, 8, 4.4, 6, 0, brick);
  mb.extrude(
    [
      [-4.4, 4.58],
      [4.4, 4.58],
      [0, 6.6],
    ],
    3.4,
    -1,
    roof,
  );
  mb.box([1.8, 2.02], 0.18, 1.4, 2.6, 0.06, 0, door);
  mb.box([-2, 2.02], 2, 1.6, 1, 0.06, 0, cream);
  // Wellhead and its motor behind the house.
  mb.cylinder([-3.5, -5.2], 0.18, 1.3, 0.9, 10, concrete);
  mb.cylinder([-3.5, -5.2], 1.08, 0.7, 1.2, 10, steel);
  // The main runs from the house to the road side, with a valve wheel.
  mb.box([3.6, 3.6], 0.5, 0.7, 0.7, 6.6, 0, pipe);
  mb.box([3.6, -1], 0.5, 0.7, 0.7, 2.6, 0, pipe);
  mb.cylinder([3.6, 5], 1.2, 0.6, 0.15, 8, steel);
  return mb.geometry();
}

/** A blue tank on four braced steel legs, with a conical cap and a ladder. */
function tower(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 10, 0.18, 10, 0, concrete);
  const legs: [number, number][] = [
    [-3.2, -3.2],
    [3.2, -3.2],
    [3.2, 3.2],
    [-3.2, 3.2],
  ];
  for (const [x, z] of legs) {
    mb.box([x, z], 0.18, 0.7, 0.5, 0.7, 0, concrete);
    mb.box([x, z], 0.68, 0.4, 14.5, 0.4, 0, steel);
  }
  // Horizontal braces at two heights.
  for (const y of [5, 10]) {
    mb.box([0, -3.2], y, 6.4, 0.25, 0.25, 0, steel);
    mb.box([0, 3.2], y, 6.4, 0.25, 0.25, 0, steel);
    mb.box([-3.2, 0], y, 0.25, 0.25, 6.4, 0, steel);
    mb.box([3.2, 0], y, 0.25, 0.25, 6.4, 0, steel);
  }
  mb.cylinder([0, 0], 0.18, 0.5, 15, 8, pipe);
  mb.cylinder([0, 0], 15, 4.6, 0.6, 16, tankDark, 4.8);
  mb.cylinder([0, 0], 15.6, 4.8, 5.4, 16, tank);
  mb.cylinder([0, 0], 21, 4.8, 2.2, 16, tankDark, 0);
  // Ladder up the road-facing leg.
  mb.box([3.2, 3.6], 0.18, 0.5, 15.4, 0.08, 0, steel);
  return mb.geometry();
}
