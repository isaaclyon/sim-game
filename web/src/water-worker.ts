// The worker's water message (ADR 0052): it asks the sim and sends what the sim says.
import type { Sim } from './pkg/sim_wasm';
import type { FromWorker } from './protocol';

type Post = (msg: FromWorker, transfer?: Transferable[]) => void;

/** Water changes with growth and tower levels, so it is coalesced like buildings. */
const INTERVAL_MS = 250;

export class WaterSender {
  private sent = -1;
  private sentAt = -Infinity;
  private readonly post: Post;

  constructor(post: Post) {
    this.post = post;
  }

  /** A new city: send everything again. */
  reset() {
    this.sent = -1;
    this.sentAt = -Infinity;
  }

  /** Sends `water` when pieces, networks, tower levels or who has water changed. */
  sendIfChanged(s: Sim, force = false) {
    const version = s.water_version();
    if (version === this.sent) return;
    const now = performance.now();
    if (!force && now - this.sentAt < INTERVAL_MS) return;
    this.sent = version;
    this.sentAt = now;
    const pieces = s.water_pieces();
    const buildings = s.water_buildings();
    this.post({ type: 'water', pieces, buildings }, [pieces.buffer, buildings.buffer]);
  }
}
