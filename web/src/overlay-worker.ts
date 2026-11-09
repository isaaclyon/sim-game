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
  private readonly post: Post;

  constructor(post: Post) {
    this.post = post;
  }

  /** A new city: send everything again. */
  reset() {
    this.trafficAt = this.servicesAt = -Infinity;
    this.services = '';
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
    if (key === this.services || (!force && now - this.servicesAt < SERVICES_INTERVAL_MS)) return;
    this.services = key;
    this.servicesAt = now;
    const lots = s.service_gaps();
    this.post({ type: 'services', lots }, [lots.buffer]);
  }
}
