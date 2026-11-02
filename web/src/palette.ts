// Scene colours from the low-poly art pass (issue #27). Albedo hex values; the light does the rest.
// Building colours live in `building-kit/style.ts`.

export const SKY = 0xd9e4e6;
export const LIGHT = {
  sky: 0xdce8ee,
  groundBounce: 0xa39878,
  hemisphere: 1.35,
  sun: 0xfff3de,
  sunIntensity: 2.1,
  /** Toward the sun in scene-local (sim x, height, sim y): high afternoon light. */
  sunFrom: [-0.38, 0.74, 0.55] as const,
};

export const GROUND = {
  grass: 0xa9bc86,
  grassLight: 0xb7c792,
  grassDark: 0x98af7b,
  meadow: 0xc3c48a,
  dirt: 0xc8ae84,
  woodsFloor: 0x8fa674,
  fieldWheat: 0xd9c37e,
  fieldCrop: 0xa8be72,
  fieldPlough: 0xb89d80,
};

export const WATER = {
  deep: 0x5e97b8,
  mid: 0x71a8c4,
  shallow: 0x8cbdcf,
  shore: 0xe0d3a8,
  bank: 0xb8ae8c,
};

export const TREES = {
  canopy: [0x6f9b58, 0x81a862, 0x5c8752],
  conifer: 0x4e7a58,
  street: 0x7aa25c,
  trunk: 0x7b5d45,
};

/** Indexed by `RoadClass`: street, main road, alley. */
export const ROADS = [0x8c8f95, 0x73777e, 0xaba597];
export const SIDEWALK = 0xdad5ca;
/** Lane paint: centre lines and road edge lines, slightly weathered. */
export const MARKINGS = { centre: 0xe2b443, edge: 0xebe8df };

/** Car tint by speed ratio: stuck → slow → free. */
export const TRAFFIC = { stuck: 0xe5383b, slow: 0xf2c14e, free: 0x38b26b };

/**
 * Zone overlay colours by `Zone` value, translucent over the ground; low density light,
 * high density dark.
 */
export const ZONE_OVERLAY = [null, 0x7be08f, 0x2fae4b, 0x7fb0ff, 0x2f6ae0, 0xf6c35a, 0xe08a1e];
export const ZONE_OVERLAY_ALPHA = 0.38;
