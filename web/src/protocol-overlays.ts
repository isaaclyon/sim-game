// Data overlays (ADR 0057, 0055): the traffic, services and schools views' worker messages. Re-exported from
// `protocol.ts`.

/** Floats per rendered segment in `traffic.segments`. */
export const TRAFFIC_STRIDE = 7;

/** Bits of a building's `gaps` in `services.lots`. */
export const Gap = {
  /** No power (ADR 0041). */
  Power: 1,
  /** A dense building with no water (ADR 0052). */
  Water: 2,
  /** No fire station reaches it, while fires can start (ADR 0053). */
  Fire: 4,
} as const;

/** Floats per school in `schoolView.schools`. */
export const SCHOOL_VIEW_STRIDE = 9;
/** Floats per segment in `schoolView.roads`. */
export const SCHOOL_ROAD_STRIDE = 5;
/** Bits of `areas` in `schoolView`: which kinds of school reach the place. */
export const SchoolArea = { Primary: 1, High: 2 } as const;

/** Worker messages for the data overlays, part of `FromWorker`. */
export type OverlayMessage =
  /**
   * `segments` is `[ax, ay, bx, by, class, road, jam]` per rendered segment of each road with
   * a car on it (`TRAFFIC_STRIDE`): class a `RoadClass`, road its stable ID, jam 0 (flowing at
   * the limit) to 1 (lanes full of stopped cars). Empty roads are absent. Sent at most once a
   * real second while running; start, edits and pause send at once.
   */
  | { type: 'traffic'; segments: Float32Array }
  /**
   * `lots` is `[gaps, n, x0, y0, …]` per standing building: its `Gap` bits, then its lot's
   * outline point count and points. Sent whole when power, water, fire, lots, buildings or
   * whether fire cover counts change; growth is coalesced like `buildings`.
   */
  | { type: 'services'; lots: Float32Array }
  /**
   * The schools view (ADR 0055). `schools` is `[x, y, half_size, front_x, front_y, kind,
   * seats, pupils, building]` per school (`SCHOOL_VIEW_STRIDE`), building its stable building
   * ID for `inspectBuilding`. `roads` is `[ax, ay, bx, by, areas]` per rendered segment of a
   * road with an end inside a school's area (`SCHOOL_ROAD_STRIDE`). `homes` is
   * `[areas, n, x0, y0, …]` per standing home: `SchoolArea` bits, then its lot's outline.
   * Sent whole when roads, schools, lots, buildings or seats taken change; growth is
   * coalesced like `buildings`.
   */
  | { type: 'schoolView'; schools: Float32Array; roads: Float32Array; homes: Float32Array };
