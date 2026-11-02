// Cars as instanced low-poly vehicles and walkers as instanced figures (ADR 0026, issue #29),
// interpolated between two agent snapshots. Keeps where each agent was drawn for picking and
// follow. Models, paint and clothes are display only, chosen from each stable agent id.

import * as THREE from 'three';
import { ensureInstances } from './instances';
import { AGENT_STRIDE, AgentKind } from './protocol';
import { VEHICLES, vehicleModel, vehiclePaint } from './vehicle-models.ts';
import { FIGURE, LIMBS, shirtOf, skinOf, swing, trousersOf } from './walker-figure.ts';

const SELECTED_COLOR = new THREE.Color(0x3fd0ff);
const WHITE = new THREE.Color(0xffffff);
const Z = new THREE.Vector3(0, 0, 1);

type Part = 'torso' | 'head' | 'leg' | 'arm';
const PARTS: Part[] = ['torso', 'head', 'leg', 'arm'];

export class AgentLayer {
  /** Per vehicle model: painted body, then trim. */
  private readonly bodies: (THREE.InstancedMesh | undefined)[] = [];
  private readonly trims: (THREE.InstancedMesh | undefined)[] = [];
  private readonly figure: Partial<Record<Part, THREE.InstancedMesh>> = {};
  private readonly material = new THREE.MeshLambertMaterial({
    vertexColors: true,
    flatShading: true,
  });
  private readonly matrix = new THREE.Matrix4();
  private readonly limb = new THREE.Matrix4();
  private readonly quat = new THREE.Quaternion();
  private readonly pos = new THREE.Vector3();
  private readonly scale = new THREE.Vector3(1, 1, 1);
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly color = new THREE.Color();
  private readonly scene: THREE.Object3D;
  /** `[x, y]` per agent id as last drawn, NaN when hidden; reused for picking and follow. */
  drawn = new Float32Array();
  /** Model index per agent id, cached because the hash runs for every car every frame. */
  private models = new Uint8Array();
  /** Colours cars by speed instead of paint: red stopped, yellow slow, green at the limit. */
  trafficView = false;

  constructor(scene: THREE.Object3D) {
    this.scene = scene;
  }

  /**
   * Draws agents interpolated between two snapshots. `t` in [0, 1] blends prev → curr.
   * Agents hidden in either snapshot snap to the visible one so spawns don't streak. Agent
   * `selected`, an agent id, is drawn in the highlight colour.
   */
  draw(prev: Float32Array | undefined, curr: Float32Array, t: number, selected: number | null) {
    const count = curr.length / AGENT_STRIDE;
    if (this.models.length < count) {
      const models = new Uint8Array(Math.ceil(count * 1.5));
      for (let i = 0; i < models.length; i++) models[i] = vehicleModel(i);
      this.models = models;
    }
    const perModel = VEHICLES.map(() => 0);
    let walkers = 0;
    for (let i = 0; i < count; i++) {
      const o = i * AGENT_STRIDE;
      if (curr[o + 3] < 0) continue;
      if (curr[o + 4] === AgentKind.Walker) walkers++;
      else perModel[this.models[i]]++;
    }
    VEHICLES.forEach((v, m) => {
      this.bodies[m] = ensureInstances(
        this.scene,
        this.bodies[m],
        perModel[m],
        v.body,
        this.material,
      );
      this.trims[m] = ensureInstances(
        this.scene,
        this.trims[m],
        perModel[m],
        v.trim,
        this.material,
      );
    });
    for (const p of PARTS) {
      const n = p === 'leg' || p === 'arm' ? walkers * 2 : walkers;
      this.figure[p] = ensureInstances(this.scene, this.figure[p], n, FIGURE[p], this.material);
    }

    if (this.drawn.length < count * 2) this.drawn = new Float32Array(count * 3);
    this.drawn.fill(Number.NaN);
    const used = VEHICLES.map(() => 0);
    let w = 0;
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
        this.drawWalker(w++, i, x * Math.cos(h) + y * Math.sin(h), ratio, i === selected);
        continue;
      }
      const m = this.models[i];
      const body = this.bodies[m] as THREE.InstancedMesh;
      const trim = this.trims[m] as THREE.InstancedMesh;
      if (i === selected) this.color.copy(SELECTED_COLOR);
      else if (this.trafficView) this.color.setHSL(Math.min(ratio, 1) * 0.33, 0.85, 0.55);
      else this.color.copy(vehiclePaint(i));
      body.setMatrixAt(used[m], this.matrix);
      body.setColorAt(used[m], this.color);
      trim.setMatrixAt(used[m], this.matrix);
      trim.setColorAt(used[m], WHITE);
      used[m]++;
    }
    VEHICLES.forEach((_, m) => {
      finish(this.bodies[m], used[m]);
      finish(this.trims[m], used[m]);
    });
    for (const p of PARTS) finish(this.figure[p], p === 'leg' || p === 'arm' ? w * 2 : w);
  }

  /** Walker `w` of this frame, agent `agent`, posed from `walked` metres along its heading. */
  private drawWalker(w: number, agent: number, walked: number, ratio: number, selected: boolean) {
    const f = this.figure as Record<Part, THREE.InstancedMesh>;
    const shirt = selected ? SELECTED_COLOR : shirtOf(agent);
    f.torso.setMatrixAt(w, this.matrix);
    f.torso.setColorAt(w, shirt);
    f.head.setMatrixAt(w, this.matrix);
    f.head.setColorAt(w, skinOf(agent));
    const angles = swing(walked, ratio, agent);
    for (const [k, p] of (['leg', 'arm'] as const).entries()) {
      const { y, z } = LIMBS[p];
      for (const [side, sign] of [
        [0, 1],
        [1, -1],
      ]) {
        this.limb.makeRotationAxis(Z, angles[k] * sign).setPosition(0, y, z[side]);
        f[p].setMatrixAt(w * 2 + side, this.limb.premultiply(this.matrix));
        f[p].setColorAt(w * 2 + side, p === 'leg' ? trousersOf(agent) : shirt);
      }
    }
  }
}

function finish(mesh: THREE.InstancedMesh | undefined, count: number) {
  if (!mesh) return;
  mesh.count = count;
  mesh.instanceMatrix.needsUpdate = true;
  if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
}
