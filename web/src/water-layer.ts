// Pumps, water towers and water markers (ADR 0052). The sim decides everything; this draws its
// `water` snapshot. Dry dense buildings always get a marker above the roof; the water view adds
// a marker coloured by network over every dense building, pump and tower, so separate
// networks stand out.
import * as THREE from 'three';
import { gridColor } from './power-layer.ts';
import { WATER_BUILDING_STRIDE, WATER_PIECE_STRIDE, type WaterKind } from './protocol.ts';
import { createWaterPiece } from './water-pieces.ts';

const DRY = new THREE.Color(0xe5484d);
const MARKER_PX = 9;
/** Markers over pumps and towers stand this high, metres. */
const PIECE_MARKER = 26;

export class WaterLayer {
  readonly group = new THREE.Group();
  private pieces = new THREE.Group();
  private markers: THREE.Points;
  private pieceData: Float32Array = new Float32Array();
  private buildings: Float32Array = new Float32Array();
  private view = false;

  constructor() {
    this.markers = new THREE.Points(
      new THREE.BufferGeometry(),
      new THREE.PointsMaterial({ size: MARKER_PX, sizeAttenuation: false, vertexColors: true }),
    );
    this.markers.renderOrder = 8;
    this.markers.frustumCulled = false;
    this.group.add(this.pieces, this.markers);
  }

  /** See `water` in protocol.ts. */
  set(pieces: Float32Array, buildings: Float32Array) {
    this.pieces.clear();
    for (let i = 0; i < pieces.length; i += WATER_PIECE_STRIDE) {
      const [kind, x, y, half, fx, fy] = pieces.subarray(i, i + 6);
      const mesh = createWaterPiece(kind as WaterKind, half);
      mesh.position.set(x, 0, y);
      // Local +z is the front; turn it to face (fx, fy) on the ground (sim y = world z).
      mesh.rotation.y = Math.atan2(fx, fy);
      this.pieces.add(mesh);
    }
    this.pieceData = pieces;
    this.buildings = buildings;
    this.setMarkers();
  }

  /** The water view colours every marker by network; off, only dry buildings show. */
  setView(on: boolean) {
    this.view = on;
    this.setMarkers();
  }

  private setMarkers() {
    const points: number[] = [];
    const colors: number[] = [];
    const c = new THREE.Color();
    const add = (x: number, top: number, y: number, color: THREE.Color) => {
      points.push(x, top, y);
      colors.push(color.r, color.g, color.b);
    };
    const b = this.buildings;
    for (let i = 0; i < b.length; i += WATER_BUILDING_STRIDE) {
      const [x, y, height, network, watered] = b.subarray(i, i + 5);
      if (!watered) add(x, height + 7, y, DRY);
      else if (this.view) add(x, height + 7, y, gridColor(network, c));
    }
    const p = this.pieceData;
    for (let i = 0; this.view && i < p.length; i += WATER_PIECE_STRIDE) {
      add(p[i + 1], PIECE_MARKER, p[i + 2], gridColor(p[i + 8], c));
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(points, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    this.markers.geometry.dispose();
    this.markers.geometry = geom;
  }
}
