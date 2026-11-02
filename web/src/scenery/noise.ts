// Deterministic display noise for scenery. Never feeds sim state.

/** Integer hash of two cell coordinates to [0, 1). */
export function hash2(i: number, k: number): number {
  let n = (Math.imul(i | 0, 7919) + Math.imul(k | 0, 104729) + 17) | 0;
  n = n ^ 61 ^ (n >>> 16);
  n = n + (n << 3);
  n = n ^ (n >>> 4);
  n = Math.imul(n, 0x27d4eb2d);
  n = n ^ (n >>> 15);
  return (n >>> 0) / 4294967296;
}

function value(x: number, z: number, s: number): number {
  const [xi, zi] = [Math.floor(x), Math.floor(z)];
  const [xf, zf] = [x - xi, z - zi];
  const u = xf * xf * (3 - 2 * xf);
  const v = zf * zf * (3 - 2 * zf);
  const a = hash2(xi + s * 131, zi);
  const b = hash2(xi + 1 + s * 131, zi);
  const c = hash2(xi + s * 131, zi + 1);
  const d = hash2(xi + 1 + s * 131, zi + 1);
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v;
}

/** Four octaves of value noise in [0, 1]; `s` picks an independent field. */
export function fbm(x: number, z: number, s = 0): number {
  let [f, a, fr] = [0, 0.5, 1];
  for (let i = 0; i < 4; i++) {
    f += a * value(x * fr, z * fr, s + i);
    a *= 0.5;
    fr *= 2.03;
  }
  return f / 0.9375;
}

/** A seeded generator (mulberry32) for placement loops. */
export function rng(seed: number): () => number {
  let a = seed | 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
