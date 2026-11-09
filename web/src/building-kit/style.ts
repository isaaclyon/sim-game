// The kit's look as data. Each era supplies a `Style` (ADR 0056); the grammar in `kit.ts`
// stays the same. Colours are albedo hex values from the art-pass design.

export interface Style {
  /** Low-density homes: pitched roofs, chimneys, a front door. */
  resLow: {
    walls: number[];
    roofs: number[];
    pitch: [number, number];
    /** Timber and dark cladding on modern homes. */
    modern: number[];
    /** Mansard and other steep slate roofs. */
    slate: number;
  };
  resHigh: {
    walls: number[];
    roof: number;
    parapet: number;
    brick: number[];
    /** Planted roof terraces. */
    garden: number;
    /** Rooftop water tanks. */
    tank: number;
  };
  comLow: { walls: number[]; roof: number; canopy: number; chrome: number };
  comHigh: { walls: number[]; roof: number; stone: number[]; copper: number };
  /** Industrial roofs are indexed by the sim's stable roof choice (0–3). */
  industry: {
    walls: number[];
    roofs: number[];
    door: number;
    stack: number;
    /** Sheet-metal sheds. */
    metal: number[];
    brick: number[];
    /** Silos, tanks and storage domes. */
    silo: number[];
    /** Roller doors on small business units. */
    doors: number[];
    /** Pipework, walkways and tower frames. */
    steel: number;
  };
  awnings: number[];
  shopGlass: number;
  glass: number;
  rooftop: number;
  chimney: number;
  door: number;
  /** The band along the ground and the door steps. */
  plinth: number;
  /** Window and door surrounds: light on darker walls, dark on light ones. */
  trim: { light: number; dark: number };
}

export const PRESENT_DAY: Style = {
  resLow: {
    walls: [0xf2e9d8, 0xeadcc4, 0xf4f1ea, 0xdce2d3, 0xe9d3c3],
    roofs: [0xb8684f, 0x8c5a47, 0x6d6f7b],
    pitch: [30, 38],
    modern: [0x5a524b, 0x8a6b4f, 0xf0efe9, 0x4f5a5e],
    slate: 0x5d626b,
  },
  resHigh: {
    walls: [0xe3d2bc, 0xc99a7c, 0xd9d6cf],
    roof: 0xcfc6b6,
    parapet: 0xb9ae9c,
    brick: [0xa65f4a, 0x8f5444, 0xb7765a],
    garden: 0x7f9a62,
    tank: 0x9a7b5c,
  },
  comLow: {
    walls: [0xeee4d0, 0xd9bfa7, 0xe0d1b3, 0xc7d0b9],
    roof: 0x929a9e,
    canopy: 0xeceae4,
    chrome: 0xc4cbd0,
  },
  comHigh: {
    walls: [0xc9d3db, 0xb7c2cc, 0xdadcd8],
    roof: 0x8f9ba8,
    stone: [0xe2d6bf, 0xd4c3a6, 0xcfc9bd],
    copper: 0x6f9e8c,
  },
  industry: {
    walls: [0xd8cdb4, 0xc9c3b5, 0xb8bfb4],
    roofs: [0x8b9398, 0x7e8b7a, 0xa08a6e, 0x6f7683],
    door: 0x6e747a,
    stack: 0xa39c92,
    metal: [0xb9c1c4, 0x8fa3a8, 0x9aa58f, 0xc7c0ae],
    brick: [0x9c5a45, 0x8a5040],
    silo: [0xd5d7d2, 0xbfc3bd, 0xa9aea8],
    doors: [0x3f6d8c, 0xb08a3e, 0x6f7f4a, 0x9b4a3c],
    steel: 0x6c7279,
  },
  awnings: [0x3f7f8e, 0x8e4f66, 0x3c557a, 0x6f8f6a],
  shopGlass: 0x55697b,
  glass: 0x62768a,
  rooftop: 0xc9ced2,
  chimney: 0x8e7f73,
  door: 0x7a5f4e,
  plinth: 0x8c877f,
  trim: { light: 0xf7f4ee, dark: 0x4d4a47 },
};

/** Village buildings (ADR 0056): limewash and fieldstone under thatch, old tile and slate,
 * with dark timber doors and no chrome. */
export const VILLAGE: Style = {
  ...PRESENT_DAY,
  resLow: {
    walls: [0xf1ead9, 0xe6d9bd, 0xd8cdb8, 0xc9b79a, 0xeee2c8],
    roofs: [0xa8935f, 0x8f7a4e, 0x9b5f45],
    pitch: [42, 50],
    modern: [0x6b5a48, 0x8a6b4f, 0xe9e1cf, 0x5d5348],
    slate: 0x5a5d63,
  },
  comLow: {
    walls: [0xe9dcc2, 0xd2b893, 0xc9b79a, 0xb8a58a],
    roof: 0x7c6b55,
    canopy: 0xd9cdb4,
    chrome: 0x6b5a48,
  },
  industry: {
    ...PRESENT_DAY.industry,
    walls: [0xc9b79a, 0xb8a58a, 0xa89b86],
    roofs: [0x7c6b55, 0x8f7a4e, 0x9b5f45, 0x5a5d63],
    door: 0x5b4636,
    metal: [0x8a7d6a, 0x7c6b55, 0x9a8b72, 0xa89b86],
    doors: [0x5b4636, 0x6b5a48, 0x4f5a3e, 0x7a4a36],
  },
  awnings: [0x7a5a3c, 0x5d6b4a, 0x8a4f3c, 0x6b5a48],
  shopGlass: 0x4d5a63,
  glass: 0x56636d,
  chimney: 0x8a7d70,
  door: 0x5b4636,
  plinth: 0x8d8475,
  trim: { light: 0xefe7d6, dark: 0x3f3a35 },
};
