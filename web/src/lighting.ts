// Display-only lighting: the simulation's secondOfDay is the only time input.
import * as THREE from 'three';
import { LIGHT, SKY } from './palette.ts';

const NIGHT_SKY = new THREE.Color(0x101c35);
const DAY_SKY = new THREE.Color(SKY);
const WARM = new THREE.Color(0xffb777);
const DAY_AMBIENT = new THREE.Color(LIGHT.sky);
const NIGHT_AMBIENT = new THREE.Color(0x7c9cdb);
const DAY_SUN = new THREE.Color(LIGHT.sun);

/** Smooth sunrise 05:00–07:00 and sunset 17:00–19:00; continuous across midnight. */
export function daylight(secondOfDay: number): number {
  const hour = (((secondOfDay % 86400) + 86400) % 86400) / 3600;
  return THREE.MathUtils.smoothstep(hour, 5, 7) * (1 - THREE.MathUtils.smoothstep(hour, 17, 19));
}

/** Per-face emission shared by tiled buildings and instanced vehicle lamps. */
export function nightMaterial(
  night: { value: number },
  lightHour?: { value: number },
): THREE.MeshLambertMaterial {
  const material = new THREE.MeshLambertMaterial({ vertexColors: true });
  material.customProgramCacheKey = () => (lightHour ? 'scheduled-night' : 'night');
  material.onBeforeCompile = (shader) => {
    shader.uniforms.night = night;
    shader.vertexShader =
      `attribute vec3 nightEmission;\nvarying vec3 vNightEmission;\n${shader.vertexShader}`.replace(
        '#include <begin_vertex>',
        '#include <begin_vertex>\nvNightEmission = nightEmission;',
      );
    if (lightHour) {
      shader.uniforms.lightHour = lightHour;
      shader.vertexShader =
        `attribute vec2 lightSchedule;\nuniform float lightHour;\n${shader.vertexShader}`.replace(
          'vNightEmission = nightEmission;',
          `float elapsed = mod(lightHour - lightSchedule.x + 24.0, 24.0);
        float duration = lightSchedule.y - lightSchedule.x;
        float lit = step(0.3, duration) * smoothstep(0.0, 0.15, elapsed)
          * (1.0 - smoothstep(max(0.0, duration - 0.15), max(0.15, duration), elapsed));
        vNightEmission = nightEmission * lit;`,
        );
    }
    shader.fragmentShader =
      `uniform float night;\nvarying vec3 vNightEmission;\n${shader.fragmentShader}`.replace(
        'vec3 totalEmissiveRadiance = emissive;',
        'vec3 totalEmissiveRadiance = emissive + vNightEmission * night;',
      );
  };
  return material;
}

export class Lighting {
  readonly night = { value: 0 };
  readonly lightHour = { value: 6 };
  readonly sky = new THREE.Color(SKY);
  readonly sun = new THREE.DirectionalLight(LIGHT.sun, LIGHT.sunIntensity);
  readonly ambient = new THREE.HemisphereLight(LIGHT.sky, LIGHT.groundBounce, LIGHT.hemisphere);
  private secondOfDay = 21600;
  private readonly target = new THREE.Vector3();

  constructor(scene: THREE.Scene) {
    scene.background = this.sky;
    this.sun.castShadow = true;
    this.sun.shadow.mapSize.set(2048, 2048);
    this.sun.shadow.bias = -0.0002;
    this.sun.shadow.normalBias = 0.3;
    scene.add(this.sun, this.sun.target, this.ambient);
  }

  setTime(secondOfDay: number) {
    this.secondOfDay = secondOfDay;
    this.lightHour.value = (((secondOfDay % 86400) + 86400) % 86400) / 3600;
  }

  /** A bounded shadow map follows the camera focus instead of covering the whole city. */
  update(scene: THREE.Scene, focus: THREE.Vector3, distance: number) {
    const day = daylight(this.secondOfDay);
    this.night.value = 1 - day;
    this.sky.copy(NIGHT_SKY).lerp(DAY_SKY, day);
    if (scene.fog) scene.fog.color.copy(this.sky);
    this.ambient.intensity = 0.38 + (LIGHT.hemisphere - 0.38) * day;
    this.ambient.color.copy(NIGHT_AMBIENT).lerp(DAY_AMBIENT, day);
    this.sun.intensity = LIGHT.sunIntensity * day;
    this.sun.color.copy(WARM).lerp(DAY_SUN, day);
    this.sun.castShadow = day > 0;
    const angle = (this.secondOfDay / 86400) * Math.PI * 2 - Math.PI / 2;
    // Focus is world space; lights live under the scene's z reflection.
    this.target.copy(focus);
    scene.worldToLocal(this.target);
    this.sun.target.position.copy(this.target);
    this.sun.position
      .copy(this.target)
      .add(new THREE.Vector3(-Math.cos(angle) * 600, Math.max(0.18, Math.sin(angle)) * 800, 350));
    const radius = THREE.MathUtils.clamp(distance * 0.7, 100, 1400);
    const camera = this.sun.shadow.camera;
    Object.assign(camera, {
      left: -radius,
      right: radius,
      top: radius,
      bottom: -radius,
      near: 1,
      far: 2400,
    });
    camera.updateProjectionMatrix();
  }
}
