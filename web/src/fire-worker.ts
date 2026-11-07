// The worker's fire messages (ADR 0053): it asks the sim and sends what the sim says.
import type { Sim } from './pkg/sim_wasm';
import type { FireStats, FromWorker, ToWorker } from './protocol';

type Post = (msg: FromWorker, transfer?: Transferable[]) => void;
type FireCommand = Extract<ToWorker, { type: 'placeStation' | 'previewStation' | 'setFires' }>;

/** Like buildings: growth can change coverage, so the message is coalesced the same way. */
const INTERVAL_MS = 250;

export class FireSender {
  private sent = '';
  private sentAt = -Infinity;
  private readonly post: Post;

  constructor(post: Post) {
    this.post = post;
  }

  /** A new city: send everything again. */
  reset() {
    this.sent = '';
    this.sentAt = -Infinity;
  }

  /** Sends `fire` when stations, fires, ruins, the setting, roads, lots or buildings changed. */
  sendIfChanged(s: Sim, force = false) {
    const key = [s.fire_version(), s.roads_version(), s.lots_version(), s.buildings_version()];
    const version = key.join();
    if (version === this.sent) return;
    const now = performance.now();
    if (!force && now - this.sentAt < INTERVAL_MS) return;
    this.sent = version;
    this.sentAt = now;
    const stations = s.fire_stations();
    const fires = s.fires();
    const ruins = s.ruins();
    const cover = s.fire_cover();
    const roadFloats = cover[0] ?? 0;
    const coverRoads = cover.slice(1, 1 + roadFloats);
    const coverBuildings = cover.slice(1 + roadFloats);
    this.post({ type: 'fire', stations, fires, ruins, coverRoads, coverBuildings }, [
      stations.buffer,
      fires.buffer,
      ruins.buffer,
      coverRoads.buffer,
      coverBuildings.buffer,
    ]);
  }

  /** Handles a fire command; true if it changed the city. */
  command(s: Sim | undefined, msg: FireCommand, ready: boolean): boolean {
    if (msg.type === 'previewStation') {
      const data = s?.station_preview(msg.x, msg.y) ?? new Float32Array();
      const outline = data.slice(1);
      this.post({ type: 'stationPreview', outline, ok: data[0] === 1, revision: msg.revision }, [
        outline.buffer,
      ]);
      return false;
    }
    if (!ready || !s) return false;
    return msg.type === 'placeStation' ? s.place_station(msg.x, msg.y) : s.set_fires(msg.on);
  }
}

/** The sim's fire figures, for `Stats.fire`. */
export function readFireStats(s: Sim): FireStats {
  const f = s.fire_stats();
  const out: FireStats = {
    enabled: f.enabled,
    threshold: f.threshold,
    stations: f.stations,
    trucks: f.trucks,
    freeTrucks: f.free_trucks,
    burning: f.burning,
    unattended: f.unattended,
    covered: f.covered,
    saved: f.saved,
    destroyed: f.destroyed,
  };
  f.free();
  return out;
}
