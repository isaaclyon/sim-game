// The kit's look as data. An era (ROADMAP M6) supplies another `Style`; the grammar in
// `kit.ts` stays the same. Colours are albedo hex values from the art-pass design.

export interface Style {
  /** Low-density homes: pitched roofs, chimneys, a front door. */
  resLow: { walls: number[]; roofs: number[]; pitch: [number, number]; gableShare: number };
  resHigh: { walls: number[]; roof: number; parapet: number };
  comLow: { walls: number[]; roof: number };
  comHigh: { walls: number[]; roof: number };
  /** Industrial roofs are indexed by the sim's stable roof choice (0–3). */
  industry: { walls: number[]; roofs: number[]; door: number; stack: number };
  awnings: number[];
  shopGlass: number;
  glass: number;
  rooftop: number;
  chimney: number;
  door: number;
}

export const PRESENT_DAY: Style = {
  resLow: {
    walls: [0xf2e9d8, 0xeadcc4, 0xf4f1ea, 0xdce2d3, 0xe9d3c3],
    roofs: [0xb8684f, 0x8c5a47, 0x6d6f7b],
    pitch: [30, 38],
    gableShare: 0.62,
  },
  resHigh: { walls: [0xe3d2bc, 0xc99a7c, 0xd9d6cf], roof: 0xcfc6b6, parapet: 0xb9ae9c },
  comLow: { walls: [0xe7e3da, 0xd7dce0, 0xe9ddcb], roof: 0xbfc6cc },
  comHigh: { walls: [0xc9d3db, 0xb7c2cc, 0xdadcd8], roof: 0x8f9ba8 },
  industry: {
    walls: [0xd8cdb4, 0xc9c3b5, 0xb8bfb4],
    roofs: [0x8b9398, 0x7e8b7a, 0xa08a6e, 0x6f7683],
    door: 0x6e747a,
    stack: 0xa39c92,
  },
  awnings: [0x3f7f8e, 0x8e4f66, 0x3c557a, 0x6f8f6a],
  shopGlass: 0x55697b,
  glass: 0x62768a,
  rooftop: 0xc9ced2,
  chimney: 0x8e7f73,
  door: 0x7a5f4e,
};
