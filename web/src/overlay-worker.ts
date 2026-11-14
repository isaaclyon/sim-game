// The worker's data overlay messages (ADR 0057): it asks the sim and sends what the sim says.
import type { Sim } from './pkg/sim_wasm';
import type { FromWorker } from './protocol';

type Post = (msg: FromWorker, transfer?: Transferable[]) => void;

/** Cars move every tick; once a real second is enough to read where traffic backs up. */
const TRAFFIC_INTERVAL_MS = 1000;
/** Service gaps change with growth, so they are coalesced like buildings. */
const SERVICES_INTERVAL_MS = 250;

export class OverlaySender {
  private trafficAt = -Infinity;
  private services = '';
  private servicesAt = -Infinity;
  private schools = '';
  private schoolsAt = -Infinity;
  private readonly post: Post;

  constructor(post: Post) {
    this.post = post;
  }

  /** A new city: send everything again. */
  reset() {
    this.trafficAt = this.servicesAt = this.schoolsAt = -Infinity;
    this.services = this.schools = '';
  }

  /** After ticks; `force` after edits, pause and start, which send at once. */
  send(s: Sim, force = false) {
    const now = performance.now();
    if (force || now - this.trafficAt >= TRAFFIC_INTERVAL_MS) {
      this.trafficAt = now;
      const segments = s.road_traffic();
      this.post({ type: 'traffic', segments }, [segments.buffer]);
    }
    const key = [
      s.power_version(),
      s.water_version(),
      s.fire_version(),
      s.lots_version(),
      s.buildings_version(),
      s.fire_gaps_shown(),
    ].join();
    if (key !== this.services && (force || now - this.servicesAt >= SERVICES_INTERVAL_MS)) {
      this.services = key;
      this.servicesAt = now;
      const lots = s.service_gaps();
      this.post({ type: 'services', lots }, [lots.buffer]);
    }
    this.sendSchools(s, force, now);
  }

  /** `schoolView`: the sim's one array is `[schools_len, roads_len, schools…, roads…, homes…]`. */
  private sendSchools(s: Sim, force: boolean, now: number) {
    const key = [
      s.roads_version(),
      s.schools_version(),
      s.lots_version(),
      s.buildings_version(),
      s.school_pupils(),
    ].join();
    if (key === this.schools || (!force && now - this.schoolsAt < SERVICES_INTERVAL_MS)) return;
    this.schools = key;
    this.schoolsAt = now;
    const all = s.school_view();
    const [a, b] = [2 + all[0], 2 + all[0] + all[1]];
    const [schools, roads, homes] = [all.slice(2, a), all.slice(a, b), all.slice(b)];
    this.post({ type: 'schoolView', schools, roads, homes }, [
      schools.buffer,
      roads.buffer,
      homes.buffer,
    ]);
  }
}
