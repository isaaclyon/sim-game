// Three.js scene: roads as line segments, vehicles as one instanced box mesh. Sim (x, y) maps
// to world (x, z) with y up.
//
// Current choices, all cheap to change:
// - WebGLRenderer. WebGPURenderer is still experimental; write new shaders in TSL so switching
//   later is easy.
// - camera-controls for the camera (MapControls was cruder at damping, bounds, zoom-to-cursor).
// - Vehicle matrices are written on the CPU each frame. If that becomes the bottleneck, move
//   interpolation and orientation into a vertex shader fed by the raw snapshot.
// - Picking is a ray against the ground plane. Add three-mesh-bvh once we pick against meshes.
// - Road meshes will likely want BatchedMesh, bezierjs for curves and earcut for polygons.

import * as THREE from 'three';
import CameraControls from 'camera-controls';

CameraControls.install({ THREE });

const STRIDE = 4;
const BG = 0x0f1419;
const STREET_COLOR = new THREE.Color(0x3a4654);
const AVENUE_COLOR = new THREE.Color(0x6b7b8f);
const CAR_LENGTH = 4.5;
const CAR_WIDTH = 2;
const CAR_HEIGHT = 1.5;

export class Renderer {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = new THREE.Scene();
  readonly camera: THREE.PerspectiveCamera;
  readonly controls: CameraControls;
  private readonly clock = new THREE.Clock();
  private roads?: THREE.LineSegments;
  private cars?: THREE.InstancedMesh;
  private readonly preview: THREE.Line;
  private readonly matrix = new THREE.Matrix4();
  private readonly quat = new THREE.Quaternion();
  private readonly pos = new THREE.Vector3();
  private readonly scale = new THREE.Vector3(1, 1, 1);
  private readonly up = new THREE.Vector3(0, 1, 0);
  private readonly color = new THREE.Color();

  constructor(container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.setSize(innerWidth, innerHeight);
    container.appendChild(this.renderer.domElement);
    this.scene.background = new THREE.Color(BG);

    this.camera = new THREE.PerspectiveCamera(45, innerWidth / innerHeight, 1, 50000);
    this.controls = new CameraControls(this.camera, this.renderer.domElement);
    // City-builder mapping: left pans, right orbits, wheel zooms toward the cursor.
    this.controls.mouseButtons.left = CameraControls.ACTION.TRUCK;
    this.controls.mouseButtons.right = CameraControls.ACTION.ROTATE;
    this.controls.dollyToCursor = true;
    this.controls.verticalDragToForward = true;
    this.controls.minPolarAngle = 0;
    this.controls.maxPolarAngle = THREE.MathUtils.degToRad(70);
    this.controls.minDistance = 30;

    this.scene.add(new THREE.HemisphereLight(0xdde6ef, 0x20262d, 2.5));
    const sun = new THREE.DirectionalLight(0xffffff, 1.2);
    sun.position.set(1, 2, 1);
    this.scene.add(sun);

    this.preview = new THREE.Line(
      new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(), new THREE.Vector3()]),
      new THREE.LineBasicMaterial({ color: 0x4a8bd0 }),
    );
    this.preview.visible = false;
    this.preview.frustumCulled = false;
    this.scene.add(this.preview);

    addEventListener('resize', () => {
      this.camera.aspect = innerWidth / innerHeight;
      this.camera.updateProjectionMatrix();
      this.renderer.setSize(innerWidth, innerHeight);
    });
  }

  /** `segments` is `[ax, ay, bx, by, speedLimit]` per road. */
  setRoads(segments: Float32Array, bounds: [number, number, number, number], fitCamera: boolean) {
    const n = segments.length / 5;
    const positions = new Float32Array(n * 6);
    const colors = new Float32Array(n * 6);
    for (let i = 0; i < n; i++) {
      const s = i * 5;
      positions.set([segments[s], 0, segments[s + 1], segments[s + 2], 0, segments[s + 3]], i * 6);
      const c = segments[s + 4] > 15 ? AVENUE_COLOR : STREET_COLOR;
      colors.set([c.r, c.g, c.b, c.r, c.g, c.b], i * 6);
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geom.setAttribute('color', new THREE.BufferAttribute(colors, 3));
    if (this.roads) {
      this.roads.geometry.dispose();
      this.roads.geometry = geom;
    } else {
      this.roads = new THREE.LineSegments(geom, new THREE.LineBasicMaterial({ vertexColors: true }));
      this.scene.add(this.roads);
    }

    if (fitCamera) {
      const [x0, y0, x1, y1] = bounds;
      const cx = (x0 + x1) / 2;
      const cz = (y0 + y1) / 2;
      const span = Math.max(x1 - x0, y1 - y0);
      this.controls.setLookAt(cx, span * 0.9, cz + span * 0.5, cx, 0, cz, false);
      this.controls.maxDistance = span * 2;
      const pad = span * 0.25;
      this.controls.setBoundary(
        new THREE.Box3(new THREE.Vector3(x0 - pad, 0, y0 - pad), new THREE.Vector3(x1 + pad, 0, y1 + pad)),
      );
    }
  }

  private ensureCars(count: number) {
    if (this.cars && this.cars.count >= count) return;
    const capacity = Math.max(count, 1024) * 1.5;
    if (this.cars) {
      this.scene.remove(this.cars);
      this.cars.dispose();
    }
    const geom = new THREE.BoxGeometry(CAR_LENGTH, CAR_HEIGHT, CAR_WIDTH);
    geom.translate(0, CAR_HEIGHT / 2, 0);
    this.cars = new THREE.InstancedMesh(geom, new THREE.MeshLambertMaterial(), Math.ceil(capacity));
    this.cars.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.cars.frustumCulled = false;
    this.scene.add(this.cars);
  }

  /**
   * Draws vehicles interpolated between two snapshots. `t` in [0, 1] blends prev → curr.
   * Agents hidden in either snapshot snap to the visible one so spawns don't streak.
   */
  drawAgents(prev: Float32Array | undefined, curr: Float32Array, t: number) {
    const count = curr.length / STRIDE;
    this.ensureCars(count);
    const cars = this.cars!;
    let drawn = 0;
    for (let i = 0; i < count; i++) {
      const o = i * STRIDE;
      const ratio = curr[o + 3];
      if (ratio < 0) continue;
      let x = curr[o];
      let y = curr[o + 1];
      let h = curr[o + 2];
      if (prev && o + 3 < prev.length && prev[o + 3] >= 0) {
        const px = prev[o];
        const py = prev[o + 1];
        // A big jump means a new trip, not motion; don't slide across the map.
        if ((x - px) ** 2 + (y - py) ** 2 < 2500) {
          x = px + (x - px) * t;
          y = py + (y - py) * t;
          let dh = h - prev[o + 2];
          dh = Math.atan2(Math.sin(dh), Math.cos(dh));
          h = prev[o + 2] + dh * t;
        }
      }
      this.pos.set(x, 0, y);
      // Sim heading is measured from +x toward +y; in world space +y became +z, so negate.
      this.quat.setFromAxisAngle(this.up, -h);
      this.matrix.compose(this.pos, this.quat, this.scale);
      cars.setMatrixAt(drawn, this.matrix);
      // red (stopped) → yellow → green (at the limit)
      this.color.setHSL(Math.min(ratio, 1) * 0.33, 0.85, 0.55);
      cars.setColorAt(drawn, this.color);
      drawn++;
    }
    cars.count = drawn;
    cars.instanceMatrix.needsUpdate = true;
    if (cars.instanceColor) cars.instanceColor.needsUpdate = true;
  }

  /** Ground-plane point under the cursor, or null if the ray misses. */
  pick(clientX: number, clientY: number): THREE.Vector3 | null {
    const ndc = new THREE.Vector2((clientX / innerWidth) * 2 - 1, -(clientY / innerHeight) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    return ray.ray.intersectPlane(new THREE.Plane(this.up, 0), new THREE.Vector3());
  }

  setPreview(a: THREE.Vector3 | null, b: THREE.Vector3 | null) {
    this.preview.visible = !!(a && b);
    if (a && b) {
      this.preview.geometry.setFromPoints([a.clone().setY(0.5), b.clone().setY(0.5)]);
    }
  }

  render() {
    this.controls.update(this.clock.getDelta());
    this.renderer.render(this.scene, this.camera);
  }
}
