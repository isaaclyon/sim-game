// Cars as instanced low-poly vehicles and walkers as instanced figures (ADR 0026, issue #29),
// interpolated between two agent snapshots. Keeps where each agent was drawn for picking and
// follow. Models, paint and clothes are display only, chosen from each stable agent id.
// Agents off screen are skipped, and far ones drawn simpler (see `agent-lod.ts`).

import * as THREE from 'three';
import { CAR_DETAIL_PX, Detail, detail, WALKER_DETAIL_PX, WALKER_HIDE_PX } from './agent-lod.ts';
import { ensureInstances } from './instances';
import { AGENT_STRIDE, AgentKind } from './protocol';
import { VEHICLES, vehicleModel, vehiclePaint } from './vehicle-models.ts';
import {
  FAR_WALKER,
  FIGURE,
  hairOf,
  hairStyle,
  LIMBS,
  shirtOf,
  sizeOf,
  skinOf,
  swing,
  trousersOf,
} from './walker-figure.ts';

const SELECTED_COLOR = new THREE.Color(0x3fd0ff);
const WHITE = new THREE.Color(0xffffff);
const Z = new THREE.Vector3(0, 0, 1);
/** A walker's height in metres, for level of detail. */
const WALKER_HEIGHT = 1.75;

type Part = keyof typeof FIGURE;
const PARTS = Object.keys(FIGURE) as Part[];
/** Instances per walker for each part; hair counts are capacities, filled per style. */
const PER_WALKER: Record<Part, number> = {
  torso: 1,
  head: 1,
  hair: 1,
  longHair: 1,
  leg: 2,
  arm: 2,
};

export class AgentLayer {
  /** Per vehicle model: painted body, then trim. */
  private readonly bodies: (THREE.InstancedMesh | undefined)[] = [];
  private readonly trims: (THREE.InstancedMesh | undefined)[] = [];
  private readonly fars: (THREE.InstancedMesh | undefined)[] = [];
  private farWalkers?: THREE.InstancedMesh;
  private readonly frustum = new THREE.Frustum();
  private readonly view = new THREE.Matrix4();
  private readonly sphere = new THREE.Sphere();
  private readonly eye = new THREE.Vector3();
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
  private readonly size = new THREE.Vector3();
  /** Walkers drawn so far this frame with short and with long hair. */
  private hairs = [0, 0];
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
   * `selected`, an agent id, is drawn in the highlight colour. Agents outside `camera`'s view
   * are skipped; `pxPerMetre` (see `pixelsPerMetre`) sets how far away detail drops.
   */
  draw(
    prev: Float32Array | undefined,
    curr: Float32Array,
    t: number,
    selected: number | null,
    camera: THREE.Camera,
    pxPerMetre: number,
  ) {
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
      this.fars[m] = ensureInstances(this.scene, this.fars[m], perModel[m], v.far, this.material);
    });
    this.farWalkers = ensureInstances(
      this.scene,
      this.farWalkers,
      walkers,
      FAR_WALKER,
      this.material,
    );
    // The camera may move again before this frame renders, so the view test keeps a margin.
    camera.updateMatrixWorld();
    // In the scene's own coordinates, which reflect z, so agent positions need no conversion.
    this.view
      .multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse)
      .multiply(this.scene.matrixWorld);
    this.frustum.setFromProjectionMatrix(this.view);
    const eye = this.scene.worldToLocal(this.eye.copy(camera.position));
    for (const p of PARTS) {
      const n = walkers * PER_WALKER[p];
      this.figure[p] = ensureInstances(this.scene, this.figure[p], n, FIGURE[p], this.material);
    }

    if (this.drawn.length < count * 2) this.drawn = new Float32Array(count * 3);
    this.drawn.fill(Number.NaN);
    const used = VEHICLES.map(() => 0);
    const usedFar = VEHICLES.map(() => 0);
    let w = 0;
    let fw = 0;
    this.hairs = [0, 0];
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
      const distance = this.pos.distanceTo(eye);
      this.sphere.set(this.pos, 4 + distance * 0.05);
      if (!this.frustum.intersectsSphere(this.sphere)) continue;
      if (kind === AgentKind.Walker) {
        const d = detail(WALKER_HEIGHT, distance, pxPerMetre, WALKER_DETAIL_PX, WALKER_HIDE_PX);
        if (d === Detail.Full)
          this.drawWalker(w++, i, x * Math.cos(h) + y * Math.sin(h), ratio, i === selected);
        else if (d === Detail.Simple) {
          const [height, build] = sizeOf(i);
          this.matrix.compose(this.pos, this.quat, this.size.set(build, height, build));
          const far = this.farWalkers as THREE.InstancedMesh;
          far.setMatrixAt(fw, this.matrix);
          far.setColorAt(fw++, i === selected ? SELECTED_COLOR : shirtOf(i));
        }
        continue;
      }
      const m = this.models[i];
      const body = this.bodies[m] as THREE.InstancedMesh;
      const trim = this.trims[m] as THREE.InstancedMesh;
      if (i === selected) this.color.copy(SELECTED_COLOR);
      else if (this.trafficView) this.color.setHSL(Math.min(ratio, 1) * 0.33, 0.85, 0.55);
      else this.color.copy(vehiclePaint(i));
      if (detail(VEHICLES[m].length, distance, pxPerMetre, CAR_DETAIL_PX) === Detail.Simple) {
        const far = this.fars[m] as THREE.InstancedMesh;
        far.setMatrixAt(usedFar[m], this.matrix);
        far.setColorAt(usedFar[m]++, this.color);
        continue;
      }
      body.setMatrixAt(used[m], this.matrix);
      body.setColorAt(used[m], this.color);
      trim.setMatrixAt(used[m], this.matrix);
      trim.setColorAt(used[m], WHITE);
      used[m]++;
    }
    VEHICLES.forEach((_, m) => {
      finish(this.bodies[m], used[m]);
      finish(this.trims[m], used[m]);
      finish(this.fars[m], usedFar[m]);
    });
    finish(this.farWalkers, fw);
    for (const p of PARTS) finish(this.figure[p], w * PER_WALKER[p]);
    finish(this.figure.hair, this.hairs[0]);
    finish(this.figure.longHair, this.hairs[1]);
  }

  /** Walker `w` of this frame, agent `agent`, posed from `walked` metres along its heading. */
  private drawWalker(w: number, agent: number, walked: number, ratio: number, selected: boolean) {
    const f = this.figure as Record<Part, THREE.InstancedMesh>;
    const shirt = selected ? SELECTED_COLOR : shirtOf(agent);
    const [height, build] = sizeOf(agent);
    this.matrix.compose(this.pos, this.quat, this.size.set(build, height, build));
    const style = hairStyle(agent);
    if (style > 0) {
      const hair = style === 1 ? f.hair : f.longHair;
      hair.setMatrixAt(this.hairs[style - 1], this.matrix);
      hair.setColorAt(this.hairs[style - 1]++, hairOf(agent));
    }
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
