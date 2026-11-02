// Cars as instanced boxes and walkers as instanced cylinders (ADR 0026), interpolated between
// two agent snapshots. Keeps where each agent was drawn for picking and follow.

import * as THREE from 'three';
import { ensureInstances, standingBox } from './instances';
import { AGENT_STRIDE, AgentKind } from './protocol';

const CAR_LENGTH = 4.5;
const CAR_WIDTH = 2;
const CAR_HEIGHT = 1.5;
const WALKER_RADIUS = 0.4;
const WALKER_HEIGHT = 1.8;
const WALKER_COLOR = new THREE.Color(0xd9b3ff);
const SELECTED_COLOR = new THREE.Color(0xffffff);

function walkerGeometry(): THREE.BufferGeometry {
  const geom = new THREE.CylinderGeometry(WALKER_RADIUS, WALKER_RADIUS, WALKER_HEIGHT, 8);
  geom.translate(0, WALKER_HEIGHT / 2, 0);
  return geom;
}

export class AgentLayer {
  private cars?: THREE.InstancedMesh;
  private walkers?: THREE.InstancedMesh;
  private readonly carMaterial = new THREE.MeshLambertMaterial();
  private readonly walkerMaterial = new THREE.MeshLambertMaterial();
  private readonly matrix = new THREE.Matrix4();
  private readonly quat = new THREE.Quaternion();
  private readonly pos = new THREE.Vector3();
  private readonly scale = new THREE.Vector3(1, 1, 1);
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly color = new THREE.Color();
  /** `[x, y]` per agent id as last drawn, NaN when hidden; reused for picking and follow. */
  drawn = new Float32Array();

  constructor(private readonly scene: THREE.Object3D) {}

  /**
   * Draws agents interpolated between two snapshots. `t` in [0, 1] blends prev → curr.
   * Agents hidden in either snapshot snap to the visible one so spawns don't streak. Agent
   * `selected`, an agent id, is drawn white.
   */
  draw(prev: Float32Array | undefined, curr: Float32Array, t: number, selected: number | null) {
    const count = curr.length / AGENT_STRIDE;
    this.cars = ensureInstances(
      this.scene,
      this.cars,
      count,
      () => standingBox([CAR_LENGTH, CAR_HEIGHT, CAR_WIDTH]),
      this.carMaterial,
    );
    this.walkers = ensureInstances(
      this.scene,
      this.walkers,
      count,
      walkerGeometry,
      this.walkerMaterial,
    );
    const { cars, walkers } = this;
    if (this.drawn.length < count * 2) this.drawn = new Float32Array(count * 3);
    this.drawn.fill(Number.NaN);
    let carCount = 0;
    let walkerCount = 0;
    for (let i = 0; i < count; i++) {
      const o = i * AGENT_STRIDE;
      const ratio = curr[o + 3];
      if (ratio < 0) continue;
      let x = curr[o];
      let y = curr[o + 1];
      let h = curr[o + 2];
      const kind = curr[o + 4];
      // Interpolate only within one trip of one kind; a big jump means a new trip, not motion.
      if (prev && o + 4 < prev.length && prev[o + 3] >= 0 && prev[o + 4] === kind) {
        const px = prev[o];
        const py = prev[o + 1];
        if ((x - px) ** 2 + (y - py) ** 2 < 2500) {
          x = px + (x - px) * t;
          y = py + (y - py) * t;
          let dh = h - prev[o + 2];
          dh = Math.atan2(Math.sin(dh), Math.cos(dh));
          h = prev[o + 2] + dh * t;
        }
      }
      this.pos.set(x, 0, y);
      // Local +z holds sim +y, so negate here; the scene reflects all positions and headings.
      this.quat.setFromAxisAngle(this.up, -h);
      this.matrix.compose(this.pos, this.quat, this.scale);
      this.drawn[i * 2] = x;
      this.drawn[i * 2 + 1] = y;
      if (kind === AgentKind.Walker) {
        walkers.setMatrixAt(walkerCount, this.matrix);
        walkers.setColorAt(walkerCount, i === selected ? SELECTED_COLOR : WALKER_COLOR);
        walkerCount++;
        continue;
      }
      // red (stopped) → yellow → green (at the limit)
      if (i === selected) this.color.copy(SELECTED_COLOR);
      else this.color.setHSL(Math.min(ratio, 1) * 0.33, 0.85, 0.55);
      cars.setMatrixAt(carCount, this.matrix);
      cars.setColorAt(carCount, this.color);
      carCount++;
    }
    for (const [mesh, n] of [
      [cars, carCount],
      [walkers, walkerCount],
    ] as const) {
      mesh.count = n;
      mesh.instanceMatrix.needsUpdate = true;
      if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    }
  }
}
