// The models behind the build menu's pictures (M4.6). Items use the game's own models: the
// building kit for zones, the plant models and the road meshes, so a change to a model changes
// its picture. Category icons are small low-poly models in the same flat-shaded style.
// Display only. Coordinates are scene-local (sim x, height, sim y); the viewer is toward sim -y.
import * as THREE from 'three';
import { buildBuildingGeometry } from '../building-kit/index.ts';
import { MeshBuilder } from '../building-kit/mesh.ts';
import type { Pt } from '../building-kit/polygon.ts';
import { createCoalPlant } from '../coal-plant.ts';
import { GROUND, ROADS, SIDEWALK, ZONE_OVERLAY } from '../palette.ts';
import { PlantKind, RoadClass, Zone } from '../protocol.ts';
import { buildRoadGeometry } from '../road-geometry.ts';
import { buildRoadMarkings } from '../road-markings.ts';
import { createSolarPlant } from '../solar-plant.ts';
import { createWindPlant } from '../wind-plant.ts';
import type { Picture } from './catalog.ts';

const lambert = new THREE.MeshLambertMaterial({ vertexColors: true });
const color = (hex: number) => new THREE.Color(hex);
const ROAD_COLORS = ROADS.map(color);
/** As in renderer.ts. */
const SIDEWALK_WIDTH = 2;
/** Pole and wire colour, as in power-layer.ts. */
const POLE = color(0x3d4048);

/**
 * One building per zone, sized as the sim grows them: width and depth in metres, height
 * (the sim's eave height) and the industrial roof choice.
 */
const BUILDINGS: Record<number, [number, number, number, number]> = {
  [Zone.ResidentialLow]: [9, 8, 3, 0],
  [Zone.ResidentialHigh]: [16, 12, 18, 0],
  [Zone.CommercialLow]: [11, 10, 4, 0],
  [Zone.CommercialHigh]: [18, 14, 20, 0],
  [Zone.IndustrialLow]: [20, 14, 7, 1],
  [Zone.IndustrialHigh]: [26, 18, 11, 2],
};

const meshOf = (mb: MeshBuilder) => new THREE.Mesh(mb.geometry(), lambert);
const group = (...parts: THREE.Object3D[]) => new THREE.Group().add(...parts);

/** A grass pad `w` × `d` metres under a model, so it stands on the ground. */
function pad(w: number, d: number): THREE.Mesh {
  const mb = new MeshBuilder();
  mb.box([0, 0], -0.4, w, 0.4, d, 0, color(GROUND.grass));
  return meshOf(mb);
}

/** The building kit's model for `zone` on its lot; the front edge faces the viewer. */
function zoneModel(zone: Zone): THREE.Object3D {
  const [w, d, height, roof] = BUILDINGS[zone];
  // Counter-clockwise, as the sim sends it; edge 0 runs along sim -y, toward the viewer.
  const ring = [-w / 2, -d / 2, w / 2, -d / 2, w / 2, d / 2, -w / 2, d / 2];
  const data = new Float32Array([zone, height, roof, 0, 4, ...ring]);
  return group(new THREE.Mesh(buildBuildingGeometry(data), lambert), pad(w + 6, d + 6));
}

/** The plant model at the sim's size, its front (local +z) turned toward the viewer. */
function plantModel(kind: PlantKind): THREE.Object3D {
  const plant = new THREE.Group();
  if (kind === PlantKind.Coal) plant.add(createCoalPlant(22));
  else if (kind === PlantKind.Solar) plant.add(createSolarPlant(16));
  else {
    const { group: turbine, rotor } = createWindPlant(5);
    rotor.rotation.z = 0.4;
    plant.add(turbine, pad(16, 16));
  }
  plant.rotation.y = Math.PI;
  return plant;
}

/** A short straight piece of road with its sidewalks and lane paint. */
function roadModel(roadClass: RoadClass, length: number): THREE.Object3D {
  const segments = new Float32Array([-length / 2, 0, length / 2, 0, roadClass]);
  // Flat layers share the ground in the game; here a small lift orders them instead.
  const flat = (geometry: THREE.BufferGeometry, material: THREE.Material, y: number) => {
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.y = y;
    return mesh;
  };
  return group(
    pad(length + 4, 26),
    flat(
      buildRoadGeometry(segments, SIDEWALK_WIDTH),
      new THREE.MeshLambertMaterial({ color: SIDEWALK }),
      0.03,
    ),
    flat(buildRoadGeometry(segments, 0, ROAD_COLORS), lambert, 0.06),
    flat(buildRoadMarkings(segments), lambert, 0.09),
  );
}

/** Two poles with cross arms and three wires, along sim x. */
function lineModel(): THREE.Object3D {
  const mb = new MeshBuilder();
  const wire = color(0x2b2b2b);
  for (const x of [-10, 10]) {
    mb.cylinder([x, 0], 0, 0.4, 14, 8, POLE);
    mb.box([x, 0], 12.4, 0.5, 0.5, 6, 0, POLE);
  }
  for (const z of [-2.5, 0, 2.5]) mb.box([0, z], 12.9, 20, 0.22, 0.22, 0, wire);
  return group(meshOf(mb), pad(30, 14));
}

/** Three slabs in the residential, commercial and industrial overlay colours. */
function zonesIcon(): THREE.Object3D {
  const mb = new MeshBuilder();
  [Zone.IndustrialLow, Zone.CommercialLow, Zone.ResidentialLow].forEach((zone, i) => {
    mb.box([0, 0], i * 2.2, 12, 1.4, 12, 0, color(ZONE_OVERLAY[zone] ?? 0));
  });
  return meshOf(mb);
}

/** A lightning bolt, as the top bar's power icon, in two convex halves. */
function powerIcon(): THREE.Object3D {
  const mb = new MeshBuilder();
  const yellow = color(0xf2c14e);
  const upper: Pt[] = [
    [13.5, 22],
    [4.5, 10.5],
    [10.7, 10.5],
    [12.7, 14],
  ];
  const lower: Pt[] = [
    [10.7, 10.5],
    [9.8, 2],
    [19, 14],
    [12.7, 14],
  ];
  for (const half of [upper, lower]) mb.extrude(half, 1.6, 0, yellow);
  return meshOf(mb);
}

/** A bulldozer facing sim +x: tracks, body, cab, exhaust and blade. */
function bulldozerIcon(): THREE.Object3D {
  const mb = new MeshBuilder();
  const yellow = color(0xe8b53a);
  const dark = color(0x3a3d42);
  for (const z of [-2.4, 2.4]) mb.box([0, z], 0, 8.5, 1.7, 1.7, 0, dark);
  mb.box([-0.3, 0], 1.5, 6.2, 2.2, 4.6, 0, yellow);
  mb.box([-1.6, 0], 3.7, 3.2, 2.9, 3.8, 0, color(0x9ec3d6));
  mb.box([-1.6, 0], 6.6, 3.6, 0.35, 4.2, 0, yellow);
  mb.cylinder([1.6, 1.2], 3.7, 0.3, 2, 8, dark);
  mb.box([5.1, 0], 0.1, 0.6, 2.8, 7, 0, color(0x8a8f96));
  for (const z of [-2.4, 2.4]) mb.box([3.9, z], 1.2, 2.2, 0.5, 0.5, 0, dark);
  return meshOf(mb);
}

/** Binoculars: two barrels and a bridge, tipped toward the viewer. */
function viewsIcon(): THREE.Object3D {
  const mb = new MeshBuilder();
  const body = color(0x3d4048);
  for (const x of [-2.3, 2.3]) {
    mb.cylinder([x, 0], 0, 1.9, 5, 10, body);
    mb.cylinder([x, 0], -0.3, 1.5, 0.3, 10, color(0x9ec3d6));
    mb.cylinder([x, 0], 5, 1.3, 2.2, 10, body);
  }
  mb.box([0, 0], 1.5, 2.8, 2.5, 1.4, 0, body);
  const binoculars = meshOf(mb);
  binoculars.rotation.x = 1.15;
  return binoculars;
}

/** A two-tone eraser lying at an angle. */
function eraserIcon(): THREE.Object3D {
  const mb = new MeshBuilder();
  mb.box([-2, 0], 0, 7, 3, 5, 0, color(0xea9aa0));
  mb.box([3.6, 0], 0, 4.4, 3.3, 5.3, 0, color(0x5b7bd6));
  const eraser = meshOf(mb);
  eraser.rotation.y = 0.35;
  return eraser;
}

/** The model a picture shows. */
export function pictureModel(p: Picture): THREE.Object3D {
  switch (p.kind) {
    case 'zone':
      return zoneModel(p.zone);
    case 'plant':
      return plantModel(p.plant);
    case 'road':
      return roadModel(p.roadClass, 28);
    case 'line':
      return lineModel();
    case 'icon':
      if (p.name === 'roads') return roadModel(RoadClass.Street, 20);
      if (p.name === 'zones') return zonesIcon();
      if (p.name === 'power') return powerIcon();
      if (p.name === 'bulldoze') return bulldozerIcon();
      if (p.name === 'views') return viewsIcon();
      return eraserIcon();
  }
}
