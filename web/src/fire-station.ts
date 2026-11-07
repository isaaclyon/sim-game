// A small flat-shaded fire station (ADR 0053). Local +z faces the road: two red engine bays
// open onto a paved apron, with a hose tower at the back corner. Visual detail only: the sim
// owns the footprint, its trucks and its coverage.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { PRESENT_DAY } from './building-kit/style.ts';

const brick = new THREE.Color(0xa4473a);
const brickDark = new THREE.Color(0x7d362d);
const concrete = new THREE.Color(0xc3baa7);
const roof = new THREE.Color(0x4c565c);
const door = new THREE.Color(0xd9d4c8);
const dark = new THREE.Color(0x354046);
const cream = new THREE.Color(0xe6ddc8);
const yellow = new THREE.Color(0xd3a04b);
const glass = new THREE.Color(PRESENT_DAY.glass);
const asphalt = new THREE.Color(0x777973);
const material = new THREE.MeshLambertMaterial({ vertexColors: true });
let geometry: THREE.BufferGeometry | undefined;

/** One shared mesh, modelled at half size 12 and scaled to the sim's footprint. */
export function createFireStation(half: number): THREE.Mesh {
  geometry ??= buildGeometry();
  const mesh = new THREE.Mesh(geometry, material);
  mesh.scale.set(half / 12, 1, half / 12);
  mesh.castShadow = mesh.receiveShadow = true;
  return mesh;
}

function buildGeometry(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 24, 0.18, 24, 0, concrete);
  // The apron in front of the bays, where trucks pull out onto the road.
  mb.box([-2, 8.5], 0.18, 15, 0.04, 7, 0, asphalt);
  for (const x of [-5.6, 1.6]) mb.box([x, 8.5], 0.22, 0.2, 0.02, 6.5, 0, yellow);
  // The engine house: two tall bays with roller doors facing the road.
  mb.box([-2, -1], 0.18, 15, 6.8, 12, 0, brick);
  mb.box([-2, -1], 6.98, 15.6, 0.45, 12.6, 0, roof);
  for (const x of [-5.6, 1.6]) {
    mb.box([x, 5.02], 0.18, 5.2, 4.8, 0.08, 0, door);
    for (let y = 1; y < 4.8; y += 0.8) mb.box([x, 5.08], y, 5.2, 0.06, 0.04, 0, dark);
    mb.box([x, 5.1], 5.1, 5.8, 0.3, 0.2, 0, cream);
  }
  mb.box([-2, 5.1], 5.9, 2, 0.7, 0.1, 0, cream);
  // A lower office wing with windows, and the hose drying tower behind it.
  mb.box([8.3, 0], 0.18, 5.4, 4.2, 10, 0, brickDark);
  mb.box([8.3, 0], 4.38, 5.8, 0.35, 10.4, 0, roof);
  for (const z of [-2.5, 2.5]) mb.box([11.03, z], 1.6, 0.06, 1.6, 2.2, 0, glass);
  mb.box([8.3, 5.03], 1.6, 2.6, 1.6, 0.06, 0, glass);
  mb.box([8, -8], 0.18, 4, 13, 4, 0, brick);
  mb.box([8, -8], 13.18, 4.6, 0.4, 4.6, 0, roof);
  for (const z of [-9, -7]) mb.box([10.04, z], 9, 0.06, 1.8, 0.9, 0, glass);
  // A siren on the roof and a hydrant by the apron.
  mb.cylinder([-2, -4], 7.43, 0.5, 1, 8, yellow);
  mb.cylinder([-10.5, 10.5], 0.22, 0.3, 0.9, 8, new THREE.Color(0xc23a2b));
  return mb.geometry();
}
