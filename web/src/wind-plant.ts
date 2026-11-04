// A tapered turbine with a separate, shared rotor mesh. The 34 m hub and 13 m blades
// retain the old silhouette; only ground equipment occupies the sim's 10 m square.
import * as THREE from 'three';
import { MeshBuilder, type V3 } from './building-kit/mesh.ts';

const white = new THREE.Color(0xe9ece6);
const trim = new THREE.Color(0xb9c5c5);
const steel = new THREE.Color(0x687b80);
const dark = new THREE.Color(0x3c4d55);
const concrete = new THREE.Color(0xc3baa7);
const red = new THREE.Color(0xa84e3d);
const material = new THREE.MeshLambertMaterial({ vertexColors: true });
let bodyGeometry: THREE.BufferGeometry | undefined;
let rotorGeometry: THREE.BufferGeometry | undefined;

export function createWindPlant(half: number): { group: THREE.Group; rotor: THREE.Mesh } {
  bodyGeometry ??= buildBody();
  rotorGeometry ??= buildRotor();
  const group = new THREE.Group();
  group.scale.set(half / 5, 1, half / 5);
  const body = new THREE.Mesh(bodyGeometry, material);
  const rotor = new THREE.Mesh(rotorGeometry, material);
  rotor.position.set(0, 34, 2.2);
  for (const mesh of [body, rotor]) mesh.castShadow = mesh.receiveShadow = true;
  group.add(body, rotor);
  return { group, rotor };
}

function buildBody(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.cylinder([0, 0], 0, 2.5, 0.35, 16, concrete);
  mb.cylinder([0, 0], 0.35, 1.5, 0.25, 16, steel);
  mb.cylinder([0, 0], 0.6, 1.25, 32.9, 16, white, 0.55);
  // Subtle bolted section seams and a collar below the housing.
  for (const y of [11, 22, 32.8]) {
    const r = 1.25 - ((y - 0.6) / 32.9) * 0.7;
    mb.cylinder([0, 0], y, r + 0.035, 0.1, 16, trim);
  }
  for (let bolt = 0; bolt < 12; bolt++) {
    const angle = (bolt * Math.PI) / 6;
    mb.cylinder([Math.cos(angle) * 1.36, Math.sin(angle) * 1.36], 0.6, 0.07, 0.1, 6, dark);
  }
  mb.box([0, 1.24], 0.6, 0.75, 2.1, 0.12, 0, steel);
  mb.box([0, 1.32], 0.68, 0.6, 1.92, 0.05, 0, white);
  mb.box([0.2, 1.37], 1.5, 0.06, 0.22, 0.05, 0, dark);
  mb.box([0, 1.9], 0, 1.2, 0.3, 1.2, 0, concrete);
  mb.box([0, 3.5], 0, 1.2, 0.12, 2, 0, concrete);

  // Chamfered nacelle (the generator housing), vents and a roof maintenance hatch.
  mb.extrude(
    [
      [-1.4, 33.2],
      [-1, 32.8],
      [1, 32.8],
      [1.4, 33.2],
      [1.4, 35.1],
      [1, 35.5],
      [-1, 35.5],
      [-1.4, 35.1],
    ],
    2,
    -0.5,
    white,
  );
  for (const x of [-1.42, 1.42])
    for (const z of [-1.6, -1.2, -0.8, -0.4]) mb.box([x, z], 33.5, 0.05, 0.9, 0.12, 0, dark);
  mb.box([0, 1.85], 33.75, 0.5, 0.5, 0.7, 0, steel);
  mb.box([0, -0.8], 35.5, 1.4, 0.12, 1.4, 0, trim);
  mb.cylinder([0, -1.7], 35.5, 0.08, 0.85, 6, steel);
  mb.cylinder([0, -1.7], 36.35, 0.16, 0.22, 8, red);

  // Transformer cabinet and protective bollards, away from the rear connection point.
  mb.box([3.35, -2.5], 0, 2.5, 0.25, 2.8, 0, concrete);
  mb.box([3.35, -2.5], 0.25, 1.8, 1.7, 2, 0, steel);
  mb.box([3.35, -2.5], 1.95, 2, 0.15, 2.2, 0, trim);
  for (const y of [0.65, 0.9, 1.15]) mb.box([3.35, -1.47], y, 1.25, 0.09, 0.05, 0, dark);
  for (const x of [2.2, 4.5]) mb.cylinder([x, -1.1], 0, 0.09, 0.8, 6, white);
  return mb.geometry();
}

function buildRotor(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  // Swept, narrowing blades with thickness that falls toward the red safety tips.
  const sections = [
    { y: 0.7, x: 0, w: 0.3, t: 0.18 },
    { y: 3, x: -0.2, w: 0.8, t: 0.2 },
    { y: 8, x: 0.15, w: 0.42, t: 0.1 },
    { y: 11.5, x: 0.45, w: 0.18, t: 0.05 },
    { y: 13, x: 0.5, w: 0.04, t: 0.025 },
  ];
  for (let blade = 0; blade < 3; blade++) {
    const angle = (blade * Math.PI * 2) / 3;
    const cs = Math.cos(angle);
    const sn = Math.sin(angle);
    const turn = (x: number, y: number, z: number): V3 => [x * cs - y * sn, x * sn + y * cs, z];
    const at = (i: number, side: number, face: number): V3 => {
      const s = sections[i];
      return turn(s.x + side * s.w, s.y, face * s.t);
    };
    for (let i = 0; i < sections.length - 1; i++) {
      const color = i === sections.length - 2 ? red : white;
      for (const face of [-1, 1])
        mb.quad(at(i, -1, face), at(i, 1, face), at(i + 1, 1, face), at(i + 1, -1, face), color, [
          0,
          0,
          face,
        ]);
      for (const side of [-1, 1])
        mb.quad(
          at(i, side, -1),
          at(i, side, 1),
          at(i + 1, side, 1),
          at(i + 1, side, -1),
          color,
          turn(side, 0, 0),
        );
    }
    for (const i of [0, sections.length - 1])
      mb.quad(
        at(i, -1, -1),
        at(i, 1, -1),
        at(i, 1, 1),
        at(i, -1, 1),
        i === 0 ? white : red,
        turn(0, i === 0 ? -1 : 1, 0),
      );
  }
  // Faceted spinner around the horizontal shaft; merged with the blades into one draw call.
  const point = (angle: number, r: number, z: number): V3 => [
    Math.cos(angle) * r,
    Math.sin(angle) * r,
    z,
  ];
  for (let i = 0; i < 12; i++) {
    const a = (i * Math.PI) / 6;
    const b = ((i + 1) * Math.PI) / 6;
    mb.quad(
      point(a, 0.9, -0.35),
      point(b, 0.9, -0.35),
      point(b, 0.22, 1.1),
      point(a, 0.22, 1.1),
      white,
      point((a + b) / 2, 1, 0),
    );
    mb.tri([0, 0, 1.1], point(a, 0.22, 1.1), point(b, 0.22, 1.1), trim, [0, 0, 1]);
    mb.tri([0, 0, -0.35], point(a, 0.9, -0.35), point(b, 0.9, -0.35), steel, [0, 0, -1]);
  }
  return mb.geometry();
}
