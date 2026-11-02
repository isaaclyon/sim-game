// Level of detail for cars and walkers: how much to draw, from how big a thing looks on screen.
// Display only; picking and follow still see every agent.
import type * as THREE from 'three';

/** Below this many pixels long, a car is drawn as a plain box. */
export const CAR_DETAIL_PX = 14;
/** Below this many pixels tall, a walker is drawn as a single block. */
export const WALKER_DETAIL_PX = 12;
/** Below this many pixels tall, a walker is not drawn at all. */
export const WALKER_HIDE_PX = 3;

export const Detail = { Hidden: 0, Simple: 1, Full: 2 } as const;

/**
 * Pixels per metre at one metre from `camera`, for a viewport `height` pixels tall; divide by
 * distance for the scale there.
 */
export function pixelsPerMetre(camera: THREE.PerspectiveCamera, height: number): number {
  return height / (2 * Math.tan((camera.fov * Math.PI) / 360));
}

/** How to draw something `size` metres across, `distance` metres away. `hide` 0 never hides. */
export function detail(size: number, distance: number, scale: number, full: number, hide = 0) {
  const px = (size * scale) / Math.max(distance, 1e-6);
  return px < hide ? Detail.Hidden : px < full ? Detail.Simple : Detail.Full;
}
