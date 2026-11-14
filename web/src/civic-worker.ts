// The worker's fire and school messages together (ADR 0053, 0055): both are placed service
// buildings the worker asks the sim about and reports as the sim says.
import { FireSender } from './fire-worker';
import type { Sim } from './pkg/sim_wasm';
import type { FromWorker, SchoolCommand, ToWorker } from './protocol';

type Post = (msg: FromWorker, transfer?: Transferable[]) => void;
type CivicCommand =
  | Extract<ToWorker, { type: 'placeStation' | 'previewStation' | 'setFires' }>
  | SchoolCommand;

export class CivicSender {
  private readonly fire: FireSender;
  private readonly post: Post;
  private schoolsSent = -1;

  constructor(post: Post) {
    this.post = post;
    this.fire = new FireSender(post);
  }

  /** A new city: send everything again. */
  reset() {
    this.fire.reset();
    this.schoolsSent = -1;
  }

  /** Sends `fire` and `schools` when what they show changed. Schools change only with an
   * edit, so they are never held back. */
  sendIfChanged(s: Sim, force = false) {
    this.fire.sendIfChanged(s, force);
    const version = s.schools_version();
    if (version === this.schoolsSent) return;
    this.schoolsSent = version;
    const schools = s.schools();
    this.post({ type: 'schools', schools }, [schools.buffer]);
  }

  /** Handles a fire or school command; true if it changed the city. */
  command(s: Sim | undefined, msg: CivicCommand, ready: boolean): boolean {
    if (msg.type === 'previewSchool') {
      const data = s?.school_preview(msg.kind, msg.x, msg.y) ?? new Float32Array();
      const outline = data.slice(2);
      const [ok, affordable] = [data[0] === 1, data[1] === 1];
      const revision = msg.revision;
      this.post({ type: 'schoolPreview', outline, ok, affordable, revision }, [outline.buffer]);
      return false;
    }
    if (msg.type !== 'placeSchool') return this.fire.command(s, msg, ready);
    return ready && !!s?.place_school(msg.kind, msg.x, msg.y);
  }
}
