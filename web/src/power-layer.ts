// Plants, transmission lines and power markers (ADR 0041). The sim decides everything; this
// draws its `power` snapshot. Unpowered buildings always get a marker above the roof; the power
// view adds a marker coloured by grid over every building, so separate grids stand out.
import * as THREE from 'three';
import { createCoalPlant } from './coal-plant.ts';
import { PLANT_STRIDE, PlantKind, POWER_BUILDING_STRIDE } from './protocol.ts';
import { createSolarPlant } from './solar-plant.ts';
import { createWindPlant } from './wind-plant.ts';

const POLE_SPACING = 40;
const POLE_HEIGHT = 14;
/** Wind rotor turns per second at full output (500), slower when the wind drops. */
const ROTOR_SPEED = 0.4 / 500;
const UNPOWERED = new THREE.Color(0xe5484d);
const MARKER_PX = 9;

const dark = new THREE.MeshLambertMaterial({ color: 0x3d4048 });
const cylinder = new THREE.CylinderGeometry(1, 1, 1, 10);

/** A distinct colour per grid number, the same for the same number. */
export function gridColor(grid: number, out = new THREE.Color()): THREE.Color {
  return out.setHSL((((grid * 0.618034) % 1) + 1) % 1, 0.75, 0.55);
}

function part(
  geometry: THREE.BufferGeometry,
  material: THREE.Material,
  x: number,
  y: number,
  z: number,
  sx: number,
  sy: number,
  sz: number,
) {
  const m = new THREE.Mesh(geometry, material);
  m.position.set(x, y, z);
  m.scale.set(sx, sy, sz);
  m.castShadow = m.receiveShadow = true;
  return m;
}

export class PowerLayer {
  readonly group = new THREE.Group();
  private plants = new THREE.Group();
  private wires: THREE.LineSegments;
  private markers: THREE.Points;
  /** Wind rotors with their output, turned in `animate`. */
  private rotors: { rotor: THREE.Object3D; output: number }[] = [];
  private buildings: Float32Array = new Float32Array();
  private view = false;

  constructor() {
    this.wires = new THREE.LineSegments(
      new THREE.BufferGeometry(),
      new THREE.LineBasicMaterial({ color: 0x2b2b2b }),
    );
    this.markers = new THREE.Points(
      new THREE.BufferGeometry(),
      new THREE.PointsMaterial({ size: MARKER_PX, sizeAttenuation: false, vertexColors: true }),
    );
    this.markers.renderOrder = 8;
    for (const o of [this.wires, this.markers]) o.frustumCulled = false;
    this.group.add(this.plants, this.wires, this.markers);
  }

  /** See `power` in protocol.ts. */
  set(plants: Float32Array, lines: Float32Array, buildings: Float32Array) {
    this.plants.clear();
    this.rotors = [];
    for (let i = 0; i < plants.length; i += PLANT_STRIDE) {
      const [kind, x, y, half, fx, fy, tx, ty, output] = plants.subarray(i, i + 9);
      const plant = this.plant(kind, x, y, half, output);
      // Local +z is the plant's front; turn it to face (fx, fy) on the ground (sim y = world z).
      plant.rotation.y = Math.atan2(fx, fy);
      this.plants.add(plant);
      this.plants.add(part(cylinder, dark, tx, POLE_HEIGHT / 2, ty, 0.35, POLE_HEIGHT, 0.35));
    }
    this.setLines(lines);
    this.buildings = buildings;
    this.setMarkers();
  }

  /** The power view colours every building's marker by grid; off, only unpowered ones show. */
  setView(on: boolean) {
    this.view = on;
    this.setMarkers();
  }

  animate(seconds: number) {
    for (const { rotor, output } of this.rotors)
      rotor.rotation.z = seconds * output * ROTOR_SPEED * Math.PI * 2;
  }

  private plant(kind: number, x: number, y: number, half: number, output: number) {
    const g = new THREE.Group();
    g.position.set(x, 0, y);
    if (kind === PlantKind.Coal) {
      g.add(createCoalPlant(half));
    } else if (kind === PlantKind.Solar) {
      g.add(createSolarPlant(half));
    } else {
      const { group, rotor } = createWindPlant(half);
      g.add(group);
      this.rotors.push({ rotor, output });
    }
    return g;
  }

  /** Poles at least every `POLE_SPACING` metres along each line, wires between their tops. */
  private setLines(lines: Float32Array) {
    const wire: number[] = [];
    for (let i = 0; i < lines.length; i += 4) {
      const [ax, ay, bx, by] = lines.subarray(i, i + 4);
      const spans = Math.max(1, Math.ceil(Math.hypot(bx - ax, by - ay) / POLE_SPACING));
      const at = (s: number) => [ax + ((bx - ax) * s) / spans, ay + ((by - ay) * s) / spans];
      for (let s = 0; s <= spans; s++) {
        const [x, y] = at(s);
        this.plants.add(part(cylinder, dark, x, POLE_HEIGHT / 2, y, 0.35, POLE_HEIGHT, 0.35));
        if (s === 0) continue;
        const [px, py] = at(s - 1);
        wire.push(px, POLE_HEIGHT, py, x, POLE_HEIGHT, y);
      }
    }
    this.wires.geometry.dispose();
    this.wires.geometry = new THREE.BufferGeometry().setAttribute(
      'position',
      new THREE.Float32BufferAttribute(wire, 3),
    );
  }

  private setMarkers() {
    const points: number[] = [];
    const colors: number[] = [];
    const c = new THREE.Color();
    const b = this.buildings;
    for (let i = 0; i < b.length; i += POWER_BUILDING_STRIDE) {
      const [x, y, height, grid, powered] = b.subarray(i, i + 5);
      if (powered && !this.view) continue;
      points.push(x, height + 4, y);
      const color = !powered ? UNPOWERED : gridColor(grid, c);
      colors.push(color.r, color.g, color.b);
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(points, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    this.markers.geometry.dispose();
    this.markers.geometry = geom;
  }
}
