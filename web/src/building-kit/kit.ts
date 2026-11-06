// The building grammar: every building is a sim footprint, height, use and front edge.
// Each use has several designs; a hash of the footprint picks one, never sim randomness, so a
// building keeps its look and the sim stays deterministic (ADR 0027). The designs live in one
// file per use: houses, flats, shops, stores, industry (workshops) and factories.
import * as THREE from 'three';
import { FACTORIES } from './factories.ts';
import { FLATS } from './flats.ts';
import { HOUSES } from './houses.ts';
import { classic, hasStack, type Smokestack, WORKSHOPS, type Works } from './industry.ts';
import { LIGHT_SEED, lightSchedule } from './lights.ts';
import type { MeshBuilder } from './mesh.ts';
import { type Archetype, type Building, seeded } from './parts.ts';
import { obb } from './polygon.ts';
import { SHOPS } from './shops.ts';
import { STORES } from './stores.ts';
import type { Style } from './style.ts';

export { houseDoorAt } from './houses.ts';
export type { Smokestack } from './industry.ts';
export { type Building, seeded } from './parts.ts';

// The classic shed takes one of four roof forms from the sim's roof choice. Listing it four
// times makes each of its forms as common as each newer design.
const CLASSIC = [classic, classic, classic, classic];

/** Designs per `Zone` value, 1 to 6. */
const DESIGNS: Archetype[][] = [
  HOUSES,
  FLATS,
  SHOPS,
  STORES,
  [...CLASSIC, ...WORKSHOPS],
  [...CLASSIC, ...FACTORIES],
];
const DESIGN_SEED = 0x64657369;

function design(b: Building): Archetype {
  const list = DESIGNS[Math.min(Math.max(Math.round(b.zone), 1), 6) - 1];
  return list[Math.floor(seeded(b.ring, DESIGN_SEED)() * list.length)];
}

/** The name of the design a building is drawn with, for tests and debugging. */
export function archetype(b: Building): string {
  return design(b).name;
}

export function building(mb: MeshBuilder, b: Building, style: Style) {
  mb.lightSchedule = lightSchedule(b.zone, seeded(b.ring, LIGHT_SEED));
  design(b).draw(mb, b, obb(b.ring), style, seeded(b.ring), new THREE.Color(style.glass));
  mb.lightSchedule = [0, 0];
}

/**
 * The chimney an industrial building draws, if any: the sim's roof choice 3 gives one. Shared
 * with the smoke layer so puffs leave the drawn stack.
 */
export function smokestack(b: Building): Smokestack | null {
  if (b.zone < 5 || !hasStack(b)) return null;
  return (design(b) as Works).stack(b, obb(b.ring));
}
