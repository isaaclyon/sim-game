// Display-only daily schedules, sampled from a building's independent footprint seed.
// Hours after midnight may exceed 24 so a night interval remains continuous.
export const LIGHT_SEED = 0x4c494748;

export function lightSchedule(zone: number, random: () => number): [number, number] {
  const use = Math.floor((zone - 1) / 2);
  if (random() >= [0.7, 0.45, 0.15][use]) return [0, 0];
  const on = 17 + random() * 2;
  // A few night owls and overnight businesses remain lit until dawn.
  const overnight = random() < 0.1;
  const off = overnight ? 29 + random() : [23, 21, 20.5][use] + random() * 2;
  return [on, off];
}
