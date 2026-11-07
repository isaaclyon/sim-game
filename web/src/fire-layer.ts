// Fire stations, flames, smoke, ruins and the fire view (ADR 0053). The sim decides
// everything; this draws its `fire` snapshot. Flames, smoke and rubble are display only.
import * as THREE from 'three';
import { MeshBuilder } from './building-kit/mesh.ts';
import type { Pt } from './building-kit/polygon.ts';
import { createFireStation } from './fire-station.ts';
import { FIRE_STRIDE, STATION_STRIDE } from './protocol.ts';
import { buildRoadGeometry } from './road-geometry.ts';

type Fire = {
  stations: Float32Array;
  fires: Float32Array;
  ruins: Float32Array;
  coverRoads: Float32Array;
  coverBuildings: Float32Array;
};

const COVER = 0x3f8fe0;
const MARKER_PX = 7;
const flame = new THREE.MeshBasicMaterial({ color: 0xff7a1a });
const core = new THREE.MeshBasicMaterial({ color: 0xffd04a });
const smoke = new THREE.MeshLambertMaterial({ color: 0x3a3a3c, transparent: true, opacity: 0.55 });
const cone = new THREE.ConeGeometry(1, 1, 6);
const puff = new THREE.IcosahedronGeometry(1, 0);
const ruinMaterial = new THREE.MeshLambertMaterial({ vertexColors: true });
const CHAR = new THREE.Color(0x2b2724);
const RUBBLE = new THREE.Color(0x5b534c);

/** One burning building's flames and smoke, animated from its own phase. */
interface Blaze {
  group: THREE.Group;
  flames: THREE.Mesh[];
  puffs: THREE.Mesh[];
  phase: number;
  size: number;
  top: number;
}

export class FireLayer {
  readonly group = new THREE.Group();
  private readonly stations = new THREE.Group();
  private readonly blazes = new THREE.Group();
  private readonly ruins: THREE.Mesh;
  private readonly cover: THREE.Mesh;
  private readonly markers: THREE.Points;
  private burning: Blaze[] = [];

  constructor() {
    this.ruins = new THREE.Mesh(new THREE.BufferGeometry(), ruinMaterial);
    this.ruins.receiveShadow = true;
    this.cover = new THREE.Mesh(
      new THREE.BufferGeometry(),
      new THREE.MeshBasicMaterial({
        color: COVER,
        transparent: true,
        opacity: 0.45,
        depthWrite: false,
      }),
    );
    this.cover.position.y = 0.05;
    this.cover.renderOrder = 6;
    this.markers = new THREE.Points(
      new THREE.BufferGeometry(),
      new THREE.PointsMaterial({
        color: COVER,
        size: MARKER_PX,
        sizeAttenuation: false,
        depthTest: false,
      }),
    );
    this.markers.renderOrder = 8;
    for (const o of [this.cover, this.markers, this.ruins]) o.frustumCulled = false;
    this.setView(false);
    this.group.add(this.stations, this.blazes, this.ruins, this.cover, this.markers);
  }

  /** See `fire` in protocol.ts. */
  set(fire: Fire) {
    this.stations.clear();
    const s = fire.stations;
    for (let i = 0; i < s.length; i += STATION_STRIDE) {
      const [x, y, half, fx, fy] = s.subarray(i, i + STATION_STRIDE);
      const station = new THREE.Group().add(createFireStation(half));
      station.position.set(x, 0, y);
      // Local +z is the front; turn it to face (fx, fy) on the ground, as plants do.
      station.rotation.y = Math.atan2(fx, fy);
      this.stations.add(station);
    }
    this.setBlazes(fire.fires);
    this.setRuins(fire.ruins);
    this.setCover(fire.coverRoads, fire.coverBuildings);
  }

  /** The fire view shows which roads and buildings a station reaches. */
  setView(on: boolean) {
    this.cover.visible = this.markers.visible = on;
  }

  /** Flames flicker and smoke rises; `seconds` is a display clock. */
  animate(seconds: number) {
    for (const b of this.burning) {
      const t = seconds + b.phase;
      b.flames.forEach((f, i) => {
        const h = b.size * (1.1 + 0.35 * Math.sin(t * 7 + i * 2.1) + 0.15 * Math.sin(t * 13 + i));
        f.scale.set(b.size * 0.45, h, b.size * 0.45);
        f.position.y = b.top + h / 2;
      });
      b.puffs.forEach((p, i) => {
        const k = (t * 0.25 + i / b.puffs.length) % 1;
        p.position.set(
          Math.sin(i * 2.4) * b.size * 0.4 + k * 3,
          b.top + b.size + k * 18,
          Math.cos(i * 2.4) * b.size * 0.4,
        );
        p.scale.setScalar(b.size * (0.4 + k * 0.9));
      });
    }
  }

  private setBlazes(fires: Float32Array) {
    this.blazes.clear();
    this.burning = [];
    for (let i = 0; i < fires.length; i += FIRE_STRIDE) {
      const [x, y, height] = fires.subarray(i, i + 3);
      const group = new THREE.Group();
      group.position.set(x, 0, y);
      const size = 2.5 + Math.min(height, 30) * 0.08;
      const flames = [-1, 0, 1].map((k) => {
        const m = new THREE.Mesh(cone, k === 0 ? core : flame);
        m.position.x = k * size * 0.5;
        m.position.z = (k * k - 0.5) * size * 0.4;
        return m;
      });
      const puffs = Array.from({ length: 5 }, () => new THREE.Mesh(puff, smoke));
      group.add(...flames, ...puffs);
      this.blazes.add(group);
      this.burning.push({ group, flames, puffs, phase: i * 0.37, size, top: height });
    }
    this.animate(0);
  }

  /** Charred ground over each ruined lot, with a few heaps of rubble. */
  private setRuins(rings: Float32Array) {
    const mb = new MeshBuilder();
    for (let i = 0; i < rings.length; ) {
      const n = rings[i++];
      const ring: Pt[] = [];
      for (let k = 0; k < n; k++, i += 2) ring.push([rings[i], rings[i + 1]]);
      mb.poly(ring, 0.06, CHAR);
      const cx = ring.reduce((sum, p) => sum + p[0], 0) / n;
      const cy = ring.reduce((sum, p) => sum + p[1], 0) / n;
      for (let k = 0; k < 4; k++)
        mb.box(
          [cx + Math.sin(k * 2.3) * 3, cy + Math.cos(k * 1.7) * 3],
          0.06,
          2.4,
          0.8 + (k % 2),
          1.8,
          k,
          RUBBLE,
        );
    }
    this.ruins.geometry.dispose();
    this.ruins.geometry = mb.geometry();
  }

  private setCover(roads: Float32Array, buildings: Float32Array) {
    // Road geometry wants `[ax, ay, bx, by, class]`; covered roads are drawn a little wider.
    const segments = new Float32Array((roads.length / 4) * 5);
    for (let i = 0, o = 0; i < roads.length; i += 4, o += 5)
      segments.set(roads.subarray(i, i + 4), o);
    this.cover.geometry.dispose();
    this.cover.geometry = buildRoadGeometry(segments, 6);
    const points: number[] = [];
    for (let i = 0; i < buildings.length; i += 2) points.push(buildings[i], 2, buildings[i + 1]);
    this.markers.geometry.dispose();
    this.markers.geometry = new THREE.BufferGeometry().setAttribute(
      'position',
      new THREE.Float32BufferAttribute(points, 3),
    );
  }
}
