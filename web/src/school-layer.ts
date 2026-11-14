// Draws the sim's schools (ADR 0055): one model per school on its site, facing its road.
import * as THREE from 'three';
import { SCHOOL_STRIDE, type SchoolKind } from './protocol.ts';
import { createSchool } from './school-building.ts';

export class SchoolLayer {
  readonly group = new THREE.Group();

  /** Takes the whole `[kind, x, y, halfSize, frontX, frontY]` snapshot. */
  set(schools: Float32Array) {
    this.group.clear();
    for (let i = 0; i < schools.length; i += SCHOOL_STRIDE) {
      const [kind, x, y, half, fx, fy] = schools.subarray(i, i + SCHOOL_STRIDE);
      const school = new THREE.Group().add(createSchool(kind as SchoolKind, half));
      school.position.set(x, 0, y);
      // Local +z is the front; turn it to face (fx, fy) on the ground, as stations do.
      school.rotation.y = Math.atan2(fx, fy);
      this.group.add(school);
    }
  }
}
