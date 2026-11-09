// Roads as flat layers: sidewalks, the carriageway and lane paint, in that draw order. The
// city's era picks the surface (ADR 0059): a village's roads are dirt tracks with trodden
// verges and no paint, a town's are asphalt, all at once when the town arrives. Display only.
import * as THREE from 'three';
import { DIRT_ROADS, ROADS, SIDEWALK, VERGE } from './palette.ts';
import { Era } from './protocol.ts';
import { buildRoadGeometry } from './road-geometry.ts';
import { buildRoadMarkings } from './road-markings.ts';

const SIDEWALK_WIDTH = 2;
const ASPHALT = ROADS.map((c) => new THREE.Color(c));
const DIRT = DIRT_ROADS.map((c) => new THREE.Color(c));

export class RoadSurface {
  readonly group = new THREE.Group();
  private segments: Float32Array = new Float32Array();
  private era: Era = Era.Town;
  private readonly sidewalkMaterial = new THREE.MeshLambertMaterial({ color: SIDEWALK });
  private readonly sidewalks = this.layer(this.sidewalkMaterial, 3);
  private readonly roads = this.layer(new THREE.MeshLambertMaterial({ vertexColors: true }), 4);
  private readonly markings = this.layer(new THREE.MeshLambertMaterial({ vertexColors: true }), 5);

  /** `segments` is `[ax, ay, bx, by, class]` per active rendered segment. */
  set(segments: Float32Array) {
    this.segments = segments;
    this.swap(this.sidewalks, buildRoadGeometry(segments, SIDEWALK_WIDTH));
    this.swap(this.markings, buildRoadMarkings(segments));
    this.paint();
  }

  /** Repaints every road for the city's era when it changes. */
  setEra(era: Era) {
    if (era === this.era) return;
    this.era = era;
    this.paint();
  }

  private paint() {
    const village = this.era === Era.Village;
    this.swap(this.roads, buildRoadGeometry(this.segments, 0, village ? DIRT : ASPHALT));
    this.sidewalkMaterial.color.set(village ? VERGE : SIDEWALK);
    this.markings.visible = !village;
  }

  private swap(mesh: THREE.Mesh, geometry: THREE.BufferGeometry) {
    mesh.geometry.dispose();
    mesh.geometry = geometry;
  }

  /** A flat transparent-pass mesh drawn at `order`. */
  private layer(material: THREE.Material, order: number): THREE.Mesh {
    Object.assign(material, { transparent: true, depthWrite: false });
    const mesh = new THREE.Mesh(new THREE.BufferGeometry(), material);
    mesh.receiveShadow = true;
    mesh.renderOrder = order;
    mesh.frustumCulled = false;
    this.group.add(mesh);
    return mesh;
  }
}
