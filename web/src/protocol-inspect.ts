// What the inspector shows about one citizen or building (ADR 0022, 0029). Re-exported from
// `protocol.ts`.
import type { Zone } from './protocol.ts';
import type { Complaint } from './protocol-services.ts';

/** What a citizen is doing or travelling to do, matching the sim's `Activity`. `SchoolRun` is
 * an adult driving their household's children to or from school (ADR 0055). */
export const Activity = { Home: 0, Work: 1, Shop: 2, School: 3, SchoolRun: 4 } as const;
export type Activity = (typeof Activity)[keyof typeof Activity];

/** How far a citizen got in school, matching the sim's `Education` (ADR 0055). */
export const Education = { None: 0, Primary: 1, HighSchool: 2, University: 3 } as const;
export type Education = (typeof Education)[keyof typeof Education];

/** A building, named by its ID and original use; 7 and 8 are the placed schools. */
export interface Place {
  building: number;
  zone: Zone | 7 | 8;
}

/**
 * Sim-owned details of one citizen. Places are null when absent, or when their building was
 * removed; all are null once the citizen has left town (`resident` false).
 */
export interface Citizen {
  id: number;
  /** Derived from the world seed and IDs; a shared surname means a shared home, not kinship. */
  name: string;
  resident: boolean;
  home: Place | null;
  job: Place | null;
  /** The current activity, or the one the current trip is for. */
  activity: Activity;
  /** Where the citizen is, or where the current trip started. */
  location: Place | null;
  /** Where the current trip goes; null while staying put. For a child riding in an adult's
   * car it is where the ride takes them: their school, or home. */
  destination: Place | null;
  /** The citizen's own travel slot: an index into `frame` agents; never reassigned. */
  agent: number | null;
  /** On a road now, so the car is drawn. */
  driving: boolean;
  /** The current trip is on foot (ADR 0026); the walker is drawn once it sets off. */
  onFoot: boolean;
  /** Age in years from the sim; a child until 18 (ADR 0054). */
  age: number;
  /** A child, who lives at home with their household. */
  child: boolean;
  /** An adult who came of age in town rather than arriving grown up (ADR 0054). */
  grewUp: boolean;
  /** A grown child still living with their family, looking for a home of their own. */
  wantsHome: boolean;
  /** Mood 0 to 100 from the sim (ADR 0058); 0 once they have left town. */
  mood: number;
  /** `Complaint` codes, in code order. */
  complaints: Complaint[];
  /** Clock minutes their last trip to work took; 0 = none yet or no job. */
  commute: number;
  /** The highest school finished (ADR 0055). */
  education: Education;
  /** The school a child holds a seat at; null for adults and children without a seat. */
  school: Place | null;
  /** A child in a household adult's car, on the way to `destination` (ADR 0055); they have
   * no car or walker of their own while riding. */
  riding: boolean;
}

/** Sim-owned counts, not physical attendance. Original use persists during closure. */
export interface Building extends Place {
  /** Resident places for homes, total jobs for workplaces (teachers for a school). */
  capacity: number;
  /** People living here or employees assigned here, including those currently elsewhere. */
  occupied: number;
  closing: boolean;
  /** Starter power or its grid covers it (ADR 0041); if not, its vacancies stay empty. */
  powered: boolean;
  /** Has water, or is low density and needs none (ADR 0052); if not, its vacancies stay
   * empty and, after a day, its people slowly leave. */
  watered: boolean;
  /** A school's seats and how many are taken (ADR 0055); null for anything else. */
  seats: number | null;
  pupils: number | null;
}
