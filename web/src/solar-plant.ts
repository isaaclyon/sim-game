// A small solar farm: framed cell arrays, racking and a rear service strip by the terminal.
// All detail stays within the sim's square footprint; local +z is the front.
import * as THREE from 'three';
import { MeshBuilder, type V3 } from './building-kit/mesh.ts';

const gravel = new THREE.Color(0xb4af96);
const concrete = new THREE.Color(0xd1c8b4);
const frame = new THREE.Color(0xb0bec1);
const steel = new THREE.Color(0x687b80);
const backing = new THREE.Color(0x303e4d);
const cells = [0x24466a, 0x2e5378, 0x294c70].map((color) => new THREE.Color(color));
const cabinet = new THREE.Color(0xd6d9cb);
const dark = new THREE.Color(0x3c4d55);
const material = new THREE.MeshLambertMaterial({ vertexColors: true });
let geometry: THREE.BufferGeometry | undefined;

export function createSolarPlant(half: number): THREE.Mesh {
  geometry ??= buildGeometry();
  const mesh = new THREE.Mesh(geometry, material);
  mesh.scale.set(half / 16, 1, half / 16);
  mesh.castShadow = mesh.receiveShadow = true;
  return mesh;
}

function buildGeometry(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 32, 0.12, 32, 0, gravel);
  // A clear maintenance aisle separates the two banks of panels.
  mb.box([0, 0], 0.12, 2.2, 0.04, 30, 0, concrete);
  mb.box([0, -13.8], 0.12, 29, 0.04, 3.1, 0, concrete);
  for (const z of [-9, -3, 3, 9]) for (const x of [-7.5, 7.5]) rack(mb, x, z);

  for (const x of [5, 8.2, 11.4]) {
    mb.box([x, -13.8], 0.16, 2.6, 0.2, 2.4, 0, concrete);
    mb.box([x, -13.8], 0.36, 2.1, 1.9, 1.8, 0, cabinet);
    mb.box([x, -13.8], 2.26, 2.3, 0.12, 2, 0, steel);
    mb.box([x, -12.87], 0.55, 0.04, 1.5, 0.04, 0, steel);
    for (const y of [1.4, 1.6, 1.8]) mb.box([x - 0.5, -12.87], y, 0.65, 0.06, 0.04, 0, dark);
    mb.box([x + 0.65, -12.84], 1.15, 0.08, 0.35, 0.08, 0, dark);
  }
  // Low utility fence; the front entry and rear terminal approach remain open.
  for (const x of [-15, 15]) {
    for (let z = -15; z <= 15; z += 5) mb.box([x, z], 0.12, 0.12, 1.5, 0.12, 0, steel);
    for (const y of [0.55, 1.45]) mb.box([x, 0], y, 0.08, 0.08, 30, 0, steel);
  }
  for (const z of [-15, 15])
    for (const x of [-8.5, 8.5]) {
      for (const y of [0.55, 1.45]) mb.box([x, z], y, 13, 0.08, 0.08, 0, steel);
      mb.box([Math.sign(x) * 2, z], 0.12, 0.12, 1.5, 0.12, 0, steel);
    }
  return mb.geometry();
}

function rack(mb: MeshBuilder, x: number, z: number) {
  const height = (depth: number) => 2 + depth * 0.35;
  const p = (across: number, depth: number, lift = 0): V3 => [
    x + across,
    height(depth) + lift,
    z + depth,
  ];
  // A closed tilted tray, not a floating sheet; cells sit just above its silver frame.
  mb.quad(p(-6, -2), p(6, -2), p(6, 2), p(-6, 2), frame, [0, 1, 0]);
  mb.quad(p(-6, -2, -0.12), p(6, -2, -0.12), p(6, 2, -0.12), p(-6, 2, -0.12), backing, [0, -1, 0]);
  for (const side of [-1, 1]) {
    mb.quad(p(side * 6, -2), p(side * 6, 2), p(side * 6, 2, -0.12), p(side * 6, -2, -0.12), frame, [
      side,
      0,
      0,
    ]);
    mb.quad(p(-6, side * 2), p(6, side * 2), p(6, side * 2, -0.12), p(-6, side * 2, -0.12), frame, [
      0,
      0,
      side,
    ]);
  }
  // Six modules across, two deep. Module borders and narrow cell gaps stay readable close up.
  for (let module = 0; module < 6; module++)
    for (let row = 0; row < 2; row++) {
      const left = -5.9 + module * 1.98;
      const back = -1.9 + row * 1.96;
      mb.quad(
        p(left, back, 0.015),
        p(left + 1.88, back, 0.015),
        p(left + 1.88, back + 1.84, 0.015),
        p(left, back + 1.84, 0.015),
        backing,
        [0, 1, 0],
      );
      for (let col = 0; col < 2; col++)
        for (let cell = 0; cell < 3; cell++) {
          const a = left + 0.04 + col * 0.92;
          const b = back + 0.04 + cell * 0.59;
          mb.quad(
            p(a, b, 0.025),
            p(a + 0.88, b, 0.025),
            p(a + 0.88, b + 0.55, 0.025),
            p(a, b + 0.55, 0.025),
            cells[(module + row + col + cell) % cells.length],
            [0, 1, 0],
          );
        }
    }
  for (const depth of [-1.5, 1.5]) {
    mb.box([x, z + depth], height(depth) - 0.24, 11.6, 0.12, 0.16, 0, steel);
    for (const across of [-4, 4]) {
      mb.box([x + across, z + depth], 0.12, 0.65, 0.18, 0.65, 0, concrete);
      mb.box([x + across, z + depth], 0.3, 0.14, height(depth) - 0.42, 0.14, 0, steel);
    }
  }
}
