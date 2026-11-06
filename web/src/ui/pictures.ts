// Draws each picture in the build menu once at startup (M4.6), with the game's WebGL renderer
// and the game's daytime light, into an offscreen target. The pixels become PNG data URLs the
// menu shows as images. Display only: nothing here reaches the sim.
import * as THREE from 'three';
import { LIGHT } from '../palette.ts';
import { createSimScene } from '../scene.ts';
import { type Picture, pictureKey } from './catalog.ts';
import { pictureModel } from './picture-models.ts';

/** CSS pixel sizes from build-menu.css: item cards and category bar icons. */
const ITEM_SIZE = [112, 76];
const ICON_SIZE = [40, 34];
/** Seen from in front and a little to the right, looking down; as the game's camera does. */
const VIEW = new THREE.Vector3(0.45, 0.62, 1).normalize();
/** The sun from the viewer's left, so fronts are lit and right sides shaded. In scene-local
 * (sim x, height, sim y); the scene reflects sim y to face the viewer. */
const SUN_FROM: [number, number, number] = [-0.55, 0.8, -0.6];
const MARGIN = 0.06;

/** Category bar icons are small and squarer; everything else is an item card. */
const isIcon = (p: Picture) => p.kind === 'icon' && p.name !== 'eraser';

/** Linear light to an sRGB byte, by table: pictures are read back from a linear target. */
const SRGB = Uint8Array.from({ length: 4096 }, (_, i) => {
  const c = i / 4095;
  return Math.round(255 * (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055));
});

/** A fresh scene with the game's daytime light. */
function stage(model: THREE.Object3D): THREE.Scene {
  const scene = createSimScene();
  const sun = new THREE.DirectionalLight(LIGHT.sun, LIGHT.sunIntensity);
  sun.position.set(...SUN_FROM).multiplyScalar(100);
  scene.add(
    new THREE.HemisphereLight(LIGHT.sky, LIGHT.groundBounce, LIGHT.hemisphere),
    sun,
    sun.target,
    model,
  );
  scene.updateMatrixWorld(true);
  return scene;
}

/** An orthographic camera along `VIEW` that fits the model's drawn shape, keeping `aspect`. */
function fit(model: THREE.Object3D, aspect: number) {
  const centre = new THREE.Box3().setFromObject(model).getCenter(new THREE.Vector3());
  const camera = new THREE.OrthographicCamera();
  camera.position.copy(centre).addScaledVector(VIEW, 1000);
  camera.lookAt(centre);
  camera.updateMatrixWorld(true);
  // Every vertex in view space, so tall thin parts such as chimneys do not pad the frame.
  const view = new THREE.Box3();
  const v = new THREE.Vector3();
  model.traverse((o) => {
    if (!(o instanceof THREE.Mesh)) return;
    const position = o.geometry.getAttribute('position');
    for (let i = 0; i < position.count; i++)
      view.expandByPoint(
        v
          .fromBufferAttribute(position, i)
          .applyMatrix4(o.matrixWorld)
          .applyMatrix4(camera.matrixWorldInverse),
      );
  });
  const size = view.getSize(new THREE.Vector3());
  const middle = view.getCenter(new THREE.Vector3());
  const half = Math.max(size.x / aspect, size.y) * (0.5 + MARGIN);
  Object.assign(camera, {
    left: middle.x - half * aspect,
    right: middle.x + half * aspect,
    top: middle.y + half,
    bottom: middle.y - half,
    near: -view.max.z - 1,
    far: -view.min.z + 1,
  });
  camera.updateProjectionMatrix();
  return camera;
}

/** Linear, premultiplied pixels (bottom row first) to straight-alpha sRGB, top row first. */
function encode(pixels: Uint16Array | Uint8Array, w: number, h: number): ImageData {
  const out = new ImageData(w, h);
  const half = pixels instanceof Uint16Array;
  const read = (i: number) => (half ? THREE.DataUtils.fromHalfFloat(pixels[i]) : pixels[i] / 255);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const i = ((h - 1 - y) * w + x) * 4;
      const o = (y * w + x) * 4;
      const a = Math.min(read(i + 3), 1);
      if (a <= 0) continue;
      for (let c = 0; c < 3; c++)
        out.data[o + c] = SRGB[Math.round(Math.min(read(i + c) / a, 1) * 4095)];
      out.data[o + 3] = Math.round(a * 255);
    }
  return out;
}

/** Renders every picture once; returns PNG data URLs by `pictureKey`. A picture that fails is
 * left out, and the menu shows its label alone. */
export function drawPictures(renderer: THREE.WebGLRenderer, pictures: Picture[]) {
  const urls = new Map<string, string>();
  const scale = Math.min(Math.max(Math.ceil(devicePixelRatio), 2), 3);
  // Half floats keep dark colours smooth; plain bytes still work where the GPU lacks them.
  const floats =
    renderer.extensions.has('EXT_color_buffer_float') ||
    renderer.extensions.has('EXT_color_buffer_half_float');
  const type = floats ? THREE.HalfFloatType : THREE.UnsignedByteType;
  const targets = new Map<string, THREE.WebGLRenderTarget>();
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('2d');
  const clear = renderer.getClearColor(new THREE.Color());
  const clearAlpha = renderer.getClearAlpha();
  const before = renderer.getRenderTarget();
  renderer.setClearColor(0x000000, 0);
  for (const picture of pictures) {
    try {
      const [w, h] = (isIcon(picture) ? ICON_SIZE : ITEM_SIZE).map((n) => n * scale);
      const key = `${w}x${h}`;
      let target = targets.get(key);
      if (!target) {
        target = new THREE.WebGLRenderTarget(w, h, { type, samples: 4 });
        targets.set(key, target);
      }
      const model = pictureModel(picture);
      const scene = stage(model);
      renderer.setRenderTarget(target);
      renderer.render(scene, fit(model, w / h));
      const pixels = floats ? new Uint16Array(w * h * 4) : new Uint8Array(w * h * 4);
      renderer.readRenderTargetPixels(target, 0, 0, w, h, pixels);
      model.traverse((o) => {
        if (o instanceof THREE.Mesh) o.geometry.dispose();
      });
      if (!context) break;
      canvas.width = w;
      canvas.height = h;
      context.putImageData(encode(pixels, w, h), 0, 0);
      urls.set(pictureKey(picture), canvas.toDataURL('image/png'));
    } catch (error) {
      console.warn('menu picture not drawn', picture, error);
    }
  }
  renderer.setRenderTarget(before);
  renderer.setClearColor(clear, clearAlpha);
  for (const target of targets.values()) target.dispose();
  return urls;
}
