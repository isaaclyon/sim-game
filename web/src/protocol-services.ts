// Power, water, fire and schools (ADR 0041, 0052, 0053, 0055): kinds, snapshot strides, stats
// and the worker messages that carry them. Re-exported from `protocol.ts`.

/** Power plant kinds, matching the sim's `PlantKind` (ADR 0041). */
export const PlantKind = {
  Coal: 0,
  Solar: 1,
  Wind: 2,
} as const;
export type PlantKind = (typeof PlantKind)[keyof typeof PlantKind];

/** Water pieces, matching the sim's `WaterKind` (ADR 0052). */
export const WaterKind = {
  Pump: 0,
  Tower: 1,
} as const;
export type WaterKind = (typeof WaterKind)[keyof typeof WaterKind];

/** School kinds, matching the sim's `SchoolKind` (ADR 0055). */
export const SchoolKind = {
  Primary: 0,
  High: 1,
} as const;
export type SchoolKind = (typeof SchoolKind)[keyof typeof SchoolKind];

/** What a placeable costs, from the sim (ADR 0036, 0052, 0053): dollars to build, and dollars
 * per clock hour to keep. */
export interface Price {
  cost: number;
  upkeep: number;
}

/** Prices of what the build menu sells besides roads, which are quoted while drawn. Power
 * lines are free. */
export interface Prices {
  /** By `PlantKind`. */
  plants: Price[];
  /** By `WaterKind`. */
  water: Price[];
  station: Price;
  /** By `SchoolKind`. */
  schools: Price[];
}

/** Floats per piece in `water.pieces`. */
export const WATER_PIECE_STRIDE = 9;
/** Floats per building in `water.buildings`. */
export const WATER_BUILDING_STRIDE = 5;

/** Floats per plant in `power.plants`. */
export const PLANT_STRIDE = 10;
/** Floats per building in `power.buildings`. */
export const POWER_BUILDING_STRIDE = 5;

/** Floats per station in `fire.stations`. */
export const STATION_STRIDE = 5;
/** Floats per burning building in `fire.fires`. */
export const FIRE_STRIDE = 4;
/** Floats per school in `schools.schools`. */
export const SCHOOL_STRIDE = 6;

/** School commands, part of `ToWorker` (ADR 0055). */
export type SchoolCommand =
  /** Places a primary or high school beside the nearest road, facing it. Refused away from
   * roads, over roads, plants, pumps, towers, stations and schools, and when the city cannot
   * pay. Buildings under it are cleared. */
  | { type: 'placeSchool'; kind: SchoolKind; x: number; y: number }
  /** Asks where a school asked for here would stand. Answered with `schoolPreview`. */
  | { type: 'previewSchool'; kind: SchoolKind; x: number; y: number; revision: number };

/** The town's power from the sim (ADR 0041), in the units of a building's demand. */
export interface PowerStats {
  /** What plants produce now on grids with at least one standing building, not counting
   * starter power. A plant no building can draw from adds nothing. */
  supply: number;
  /** The small supply every city starts with, given to its oldest buildings. */
  starter: number;
  /** Demand of the powered buildings. */
  used: number;
  /** Demand of every standing building. */
  demand: number;
  unpowered: number;
  /** Scheduled lots whose last growth check failed for power, preserved across loads. */
  waiting: number;
}

/** The town's water from the sim (ADR 0052), in the units of a building's use. Only dense
 * buildings use water. */
export interface WaterStats {
  /** What all pumps give each update now. */
  supply: number;
  /** Use of the dense buildings that get water, from pumps or towers. */
  used: number;
  /** Use of every standing dense building. */
  demand: number;
  /** Water in all towers, and what they hold when full. */
  stored: number;
  capacity: number;
  /** Standing dense buildings without water. */
  dry: number;
  /** Scheduled lots whose last growth check failed for water, preserved across loads. */
  waiting: number;
}

/** The city's fire figures from the sim (ADR 0053). */
export interface FireStats {
  /** The city's fire setting; off, no fire starts. */
  enabled: boolean;
  /** No fire starts while fewer people than this live in the city. */
  threshold: number;
  stations: number;
  trucks: number;
  /** Trucks at their station, free to go. */
  freeTrucks: number;
  burning: number;
  /** Burning with no truck on the way. */
  unattended: number;
  /** Standing buildings a station covers. */
  covered: number;
  saved: number;
  destroyed: number;
}

/**
 * What makes a citizen unhappy (ADR 0058), as `sim::world::Complaint` codes: 0 no power,
 * 1 no water, 2 no fire station nearby, 3 dirty air, 4 noise, 5 no job, 6 long commute.
 */
export type Complaint = 0 | 1 | 2 | 3 | 4 | 5 | 6;

/** The city's mood from the sim (ADR 0058). */
export interface HappinessStats {
  /** Residents' mean mood, 0 to 100; 50 with nobody in town. */
  average: number;
  /** Residents with each complaint, indexed by `Complaint` code. */
  complaints: number[];
}

/** Worker messages for power, water and fire, part of `FromWorker`. */
export type ServiceMessage =
  /**
   * Power (ADR 0041), sent whole when plants, lines, grids or who is powered change; growth
   * is coalesced like `buildings`. `plants` is `[kind, x, y, halfSize, frontX, frontY,
   * terminalX, terminalY, output, grid]` per plant (`PLANT_STRIDE`): its footprint is the
   * square `halfSize` around (x, y), turned so its front faces the unit direction (frontX,
   * frontY) (toward its road for coal; ADR 0042); lines join it at the terminal. `lines` is
   * `[ax, ay, bx, by]` per line. `buildings` is `[x, y, height, grid, powered]` per standing
   * building (`POWER_BUILDING_STRIDE`): footprint mean point, metres, grid (-1 = none), 1 or 0.
   * Grid numbers are only stable within one message.
   */
  | { type: 'power'; plants: Float32Array; lines: Float32Array; buildings: Float32Array }
  /**
   * Water (ADR 0052), sent whole when pieces, networks, tower levels or who has water change;
   * growth is coalesced like `buildings`. `pieces` is `[kind, x, y, halfSize, frontX, frontY,
   * output, level, network]` per pump or tower (`WATER_PIECE_STRIDE`): its square footprint
   * faces (frontX, frontY), toward its road; output is what a pump gives now (0 for towers),
   * level what a tower holds (0 for pumps). `buildings` is `[x, y, height, network, watered]`
   * per standing dense building (`WATER_BUILDING_STRIDE`): footprint mean point, metres,
   * network (-1 = none), 1 or 0. Network numbers are only stable within one message.
   */
  | { type: 'water'; pieces: Float32Array; buildings: Float32Array }
  /** `outline` is the closed `[x, y]` footprint the pump or tower asked about would take,
   * snapped beside a road; `ok` if a click would place it (it fits and the city can pay),
   * `affordable` false when the city cannot pay. */
  | PlacementPreview<'waterPreview'>
  /** `outline` is the closed `[x, y]` footprint the plant asked about would take, snapped
   * beside a road for coal (ADR 0042); `ok` and `affordable` as for `waterPreview`. */
  | PlacementPreview<'plantPreview'>
  /**
   * Fire (ADR 0053), sent whole when stations, fires, ruins, the fire setting, roads or
   * buildings change; growth is coalesced like `buildings`. `stations` is `[x, y, halfSize,
   * frontX, frontY]` per station (`STATION_STRIDE`): a square turned to face its road.
   * `fires` is `[x, y, height, truck]` per burning building (`FIRE_STRIDE`), oldest first:
   * footprint mean point, metres, 1 if a truck is on the way. `ruins` is `[n, x0, y0, …]` per
   * lot holding ruins, like `lots`. `coverRoads` is `[ax, ay, bx, by]` per rendered segment of
   * a road a station reaches; `coverBuildings` is `[x, y]` per covered building.
   */
  | {
      type: 'fire';
      stations: Float32Array;
      fires: Float32Array;
      ruins: Float32Array;
      coverRoads: Float32Array;
      coverBuildings: Float32Array;
    }
  /** `outline` is the closed `[x, y]` footprint a station asked about would take, beside its
   * road, empty where no road is in reach; `ok` and `affordable` as for `waterPreview`. */
  | PlacementPreview<'stationPreview'>
  /**
   * Schools (ADR 0055), sent whole when one is placed or removed: `[kind, x, y, halfSize,
   * frontX, frontY]` per school (`SCHOOL_STRIDE`), kind as `SchoolKind`: a square site turned
   * to face its road. Each school is also a building in `buildings`, zone 7 = primary school,
   * 8 = high school, which the page draws from this message instead.
   */
  | { type: 'schools'; schools: Float32Array }
  /** `outline` is the closed `[x, y]` site a school asked about would take, beside its road,
   * empty where no road is in reach; `ok` and `affordable` as for `waterPreview`. */
  | PlacementPreview<'schoolPreview'>
  /** `[ax, ay, aJoin, bx, by, bJoin]`, join 1 where an end meets a line, plant or road and 0
   * where it stands free; empty means refused. */
  | { type: 'linePreview'; ends: Float32Array; revision: number };

/** Where a plant, pump, tower, station or school asked about would stand, and whether it would. */
export interface PlacementPreview<T> {
  type: T;
  outline: Float32Array;
  ok: boolean;
  affordable: boolean;
  revision: number;
}
