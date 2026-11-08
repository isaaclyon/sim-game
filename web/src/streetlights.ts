// Streetlights along every road, display only like street trees (ADR 0027). Placement comes
// from the roads snapshot alone; the lamps glow and light a pool on the ground at night.
//
// Models stand on y = 0 with the arm along +x, toward the road; the scene reflects z.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import { drawFirst, ensureInstances } from './instances.ts';
import { Clearance } from './scenery/clearance.ts';

/** Metres between lamps along a road, alternating sides, by class: street, main road, alley. */
const STEP = [30, 15, 45];
/** Asphalt half-widths by class, as drawn; poles stand `INSET` onto the sidewalk. */
const ASPHALT = [4, 6, 4];
const INSET = 0.4;
/** No lamp this close to a junction where three or more segments meet, clear of its signals. */
const JUNCTION_CLEAR = 12;
const ARM = 1.8;
const HEIGHT = 7;
const POOL_RADIUS = 14;

const STEEL = new THREE.Color(0x6f747a);
const HOUSING = new THREE.Color(0x3a3d42);
const LENS = new THREE.Color(0xfff1d6);
const GLOW = new THREE.Color(0xffd08a);

/** `[x, y, heading]` per lamp; heading is the sim angle from the pole toward the road. */
export function placeStreetlights(segments: Float32Array): Float32Array {
  const ends = new Map<string, number>();
  for (let i = 0; i < segments.length; i += 5)
    for (const k of [0, 2]) {
      const key = `${segments[i + k]},${segments[i + k + 1]}`;
      ends.set(key, (ends.get(key) ?? 0) + 1);
    }
  const junction = (x: number, y: number) => (ends.get(`${x},${y}`) ?? 0) >= 3;
  const clearance = new Clearance();
  clearance.setRoads(segments);

  const out: number[] = [];
  let chain = 0;
  let last: number[] = [];
  for (let i = 0; i < segments.length; i += 5) {
    const [ax, ay, bx, by, cls] = segments.subarray(i, i + 5);
    const length = Math.hypot(bx - ax, by - ay);
    // Curves arrive as consecutive samples; keep the spacing across them.
    const continues = last[0] === ax && last[1] === ay && last[2] === cls;
    const start = continues ? chain : 0;
    chain = start + length;
    last = [bx, by, cls];
    if (length === 0) continue;
    const [ux, uy] = [(bx - ax) / length, (by - ay) / length];
    const step = STEP[cls] ?? STEP[0];
    const half = (ASPHALT[cls] ?? ASPHALT[0]) + INSET;
    const [nearA, nearB] = [junction(ax, ay), junction(bx, by)];
    for (let k = Math.floor(start / step) + 1; k * step <= chain; k++) {
      const s = k * step - start;
      if ((nearA && s < JUNCTION_CLEAR) || (nearB && length - s < JUNCTION_CLEAR)) continue;
      const side = k % 2 ? 1 : -1;
      const [x, y] = [ax + ux * s - uy * half * side, ay + uy * s + ux * half * side];
      // Closer to some other road than to its own: inside a junction or on other asphalt.
      if (clearance.roadGap(x, y) < INSET / 2) continue;
      out.push(x, y, Math.atan2(-ux * side, uy * side));
    }
  }
  return new Float32Array(out);
}

/** Pole, arm and head; only the lens under the head emits at night. */
export function LAMP_POST(): THREE.BufferGeometry {
  const mb = new MeshBuilder();
  mb.box([0, 0], 0, 0.3, 0.4, 0.3, 0, STEEL);
  mb.cylinder([0, 0], 0, 0.08, HEIGHT, 6, STEEL, 0.06);
  mb.box([ARM / 2, 0], HEIGHT - 0.1, ARM, 0.08, 0.08, 0, STEEL);
  mb.box([ARM, 0], HEIGHT - 0.2, 0.75, 0.18, 0.34, 0, HOUSING);
  mb.glow.copy(GLOW);
  mb.box([ARM, 0], HEIGHT - 0.3, 0.6, 0.1, 0.26, 0, LENS);
  return mb.geometry();
}

/** A flat fan under the lens, bright in the middle and black (adds nothing) at the rim. */
export function LIGHT_POOL(): THREE.BufferGeometry {
  const sides = 24;
  // Brightness falls off with the square of the distance left to the rim: a long, soft edge.
  const rings = Array.from({ length: 9 }, (_, k): [number, number] => {
    const f = k / 8;
    return [POOL_RADIUS * f, (1 - f) ** 2];
  });
  const at = (r: number, i: number) => {
    const a = (i / sides) * Math.PI * 2;
    return [ARM + Math.cos(a) * r, 0.03, Math.sin(a) * r];
  };
  const positions: number[] = [];
  const colors: number[] = [];
  const vertex = ([r, c]: [number, number], i: number) => {
    positions.push(...at(r, i));
    colors.push(c, c, c);
  };
  for (let i = 0; i < sides; i++)
    for (let ring = 0; ring < rings.length - 1; ring++) {
      const [inner, outer] = [rings[ring], rings[ring + 1]];
      // Counterclockwise from above, so front faces point up.
      vertex(inner, i);
      vertex(outer, i + 1);
      vertex(outer, i);
      if (ring === 0) continue;
      vertex(inner, i);
      vertex(inner, i + 1);
      vertex(outer, i + 1);
    }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
  return g;
}

export class StreetlightLayer {
  readonly group = new THREE.Group();
  private posts?: THREE.InstancedMesh;
  private pools?: THREE.InstancedMesh;
  private readonly postMaterial: THREE.Material;
  /** Additive and unlit: warm light added to whatever lies under it, scaled by `opacity`. */
  private readonly poolMaterial = new THREE.MeshBasicMaterial({
    color: 0xffb867,
    vertexColors: true,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    fog: false,
    opacity: 0,
  });

  /** `postMaterial` should glow with night emission, as from `nightMaterial`. */
  constructor(postMaterial: THREE.Material) {
    this.postMaterial = postMaterial;
  }

  /** Re-places every lamp from `[ax, ay, bx, by, class]` per rendered road segment. */
  set(segments: Float32Array) {
    const lamps = placeStreetlights(segments);
    const count = lamps.length / 3;
    this.posts = ensureInstances(this.group, this.posts, count, LAMP_POST, this.postMaterial);
    this.pools = ensureInstances(this.group, this.pools, count, LIGHT_POOL, this.poolMaterial);
    this.posts.castShadow = true;
    // Draw after lane paint (render order 5) so pools light the markings too.
    this.pools.renderOrder = 6;
    const [m, q, p, s, up] = [
      new THREE.Matrix4(),
      new THREE.Quaternion(),
      new THREE.Vector3(),
      new THREE.Vector3(1, 1, 1),
      new THREE.Vector3(0, 1, 0),
    ];
    for (let i = 0; i < count; i++) {
      p.set(lamps[i * 3], 0, lamps[i * 3 + 1]);
      // Local +z holds sim +y, so negate here; the scene reflects all positions and headings.
      q.setFromAxisAngle(up, -lamps[i * 3 + 2]);
      m.compose(p, q, s);
      this.posts.setMatrixAt(i, m);
      this.pools.setMatrixAt(i, m);
    }
    for (const mesh of [this.posts, this.pools]) drawFirst(mesh, count);
  }

  /** `on` runs from 0 (off) to 1 (fully lit); see `Lighting.streetlights`. */
  update(on: number) {
    // Wide pools overlap on main roads, so each one is dimmer than a lone lamp would need.
    this.poolMaterial.opacity = on * 0.32;
    if (this.pools) this.pools.visible = on > 0;
  }
}
