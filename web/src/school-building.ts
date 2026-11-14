// Small flat-shaded schools (ADR 0055). Local +z faces the road. The primary school is one
// low yellow building with a bell and a playground; the high school is two brick storeys
// around a forecourt, with a ball court. Visual detail only: the sim owns the site, the seats
// and who goes there.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { PRESENT_DAY } from './building-kit/style.ts';
import { SchoolKind } from './protocol.ts';

const grass = new THREE.Color(0x8fae6b);
const path = new THREE.Color(0xcfc6b2);
const asphalt = new THREE.Color(0x7c7e78);
const line = new THREE.Color(0xe9e4d6);
const yellow = new THREE.Color(0xe2bd62);
const teal = new THREE.Color(0x3f7f86);
const brick = new THREE.Color(0xb9825a);
const brickDark = new THREE.Color(0x996748);
const slate = new THREE.Color(0x55606b);
const cream = new THREE.Color(0xeee6d2);
const door = new THREE.Color(0x3d6aa3);
const red = new THREE.Color(0xc8503f);
const pole = new THREE.Color(0xd8d8d2);
const glass = new THREE.Color(PRESENT_DAY.glass);
const material = new THREE.MeshLambertMaterial({ vertexColors: true });
/** The half size each kind is modelled at: the sim's own site sizes. */
const MODEL_HALF = [14, 18];
const geometries: (THREE.BufferGeometry | undefined)[] = [];

/** One shared mesh per kind, scaled to the sim's site when it differs from the model's. */
export function createSchool(kind: SchoolKind, half = MODEL_HALF[kind]): THREE.Mesh {
  geometries[kind] ??= kind === SchoolKind.High ? highSchool() : primarySchool();
  const mesh = new THREE.Mesh(geometries[kind], material);
  mesh.scale.set(half / MODEL_HALF[kind], 1, half / MODEL_HALF[kind]);
  mesh.castShadow = mesh.receiveShadow = true;
  return mesh;
}

/** A flag on a pole. */
function flag(mb: MeshBuilder, x: number, z: number) {
  mb.cylinder([x, z], 0.2, 0.12, 7.5, 6, pole);
  mb.box([x + 0.9, z], 6.3, 1.7, 1.1, 0.08, 0, red);
}

function primarySchool(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 28, 0.18, 28, 0, grass);
  // The classrooms: one long low block at the back, under a teal roof.
  mb.box([0, -6], 0.18, 24, 4.2, 10, 0, yellow);
  mb.box([0, -6], 4.38, 25, 0.5, 11, 0, teal);
  mb.box([0, -6], 4.88, 21, 0.5, 7, 0, teal);
  for (let x = -10; x <= 10; x += 4)
    if (x !== -2 && x !== 2) mb.box([x, -0.97], 1.4, 2.6, 1.7, 0.08, 0, glass);
  // The entrance, with a porch and a bell above it.
  mb.box([0, -0.4], 0.18, 5, 4.9, 1.6, 0, cream);
  mb.box([0, 0.42], 0.18, 2, 2.6, 0.08, 0, door);
  mb.box([0, -6], 5.38, 2.2, 2.4, 2.2, 0, cream);
  mb.box([0, -6], 7.78, 2.8, 0.4, 2.8, 0, teal);
  mb.box([0, -4.88], 6.1, 0.9, 0.9, 0.08, 0, red);
  // The path to the road, and a playground beside it.
  mb.box([0, 7.2], 0.18, 3, 0.04, 13.6, 0, path);
  mb.box([-8, 7], 0.18, 10, 0.04, 9, 0, asphalt);
  for (const z of [4.5, 7, 9.5]) mb.box([-8, z], 0.22, 7, 0.02, 0.2, 0, line);
  mb.box([8, 6], 0.18, 6, 0.3, 6, 0, path);
  mb.box([8, 6], 0.48, 0.3, 2.4, 3, 0, red);
  mb.box([8, 6], 2.88, 3.4, 0.2, 0.3, 0, red);
  flag(mb, 4, 11.5);
  return mb.geometry();
}

function highSchool(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 36, 0.18, 36, 0, grass);
  // Two storeys of classrooms across the back, and a wing forward on each side.
  mb.box([0, -9], 0.18, 32, 7.4, 10, 0, brick);
  mb.box([0, -9], 7.58, 32.8, 0.45, 10.8, 0, slate);
  for (const x of [-11.5, 11.5]) {
    mb.box([x, 1], 0.18, 9, 7.4, 10, 0, brickDark);
    mb.box([x, 1], 7.58, 9.8, 0.45, 10.8, 0, slate);
    for (const y of [1.5, 4.7])
      for (const dx of [-2.2, 2.2]) mb.box([x + dx, 6.03], y, 2.6, 1.7, 0.08, 0, glass);
  }
  for (const y of [1.5, 4.7])
    for (let x = -5; x <= 5; x += 3.3)
      if (y > 2 || Math.abs(x) > 2) mb.box([x, -3.97], y, 2.3, 1.7, 0.08, 0, glass);
  // The entrance at the back of the forecourt, under a cream canopy.
  mb.box([0, -3.2], 3.4, 6, 0.4, 2.4, 0, cream);
  for (const x of [-2.6, 2.6]) mb.box([x, -2.3], 0.18, 0.4, 3.2, 0.4, 0, cream);
  mb.box([0, -3.95], 0.18, 3, 2.8, 0.08, 0, door);
  mb.box([0, 5], 0.18, 13, 0.04, 26, 0, path);
  // A ball court along the road, and the flag.
  mb.box([-11.5, 12], 0.18, 10, 0.04, 9, 0, asphalt);
  mb.box([-11.5, 12], 0.22, 8.4, 0.02, 0.2, 0, line);
  for (const x of [-15.7, -7.3]) mb.box([x, 12], 0.22, 0.2, 0.02, 7.4, 0, line);
  for (const x of [-15.9, -7.1]) mb.box([x, 12], 0.18, 0.15, 3.2, 0.15, 0, pole);
  flag(mb, 9, 14);
  return mb.geometry();
}
