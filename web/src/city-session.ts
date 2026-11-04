// Persistence lifecycle runs in the existing worker; JavaScript never decodes sim state.
import { type CityStorage, cityStorage } from './city-storage';
import { Sim } from './pkg/sim_wasm';
import type { CityStatus, ToWorker } from './protocol';

type Options = Extract<ToWorker, { type: 'init' }>;
const AUTOSAVE_MS = 60_000;

export class CitySession {
  ready = false;
  private sim?: Sim;
  private options?: Options;
  private revision = 0;
  private savedRevision = 0;
  private savedAt: number | null = null;
  private error: string | null = null;
  private operations = 0;
  private queue = Promise.resolve();
  private readonly post: (status: CityStatus) => void;
  private readonly install: (sim: Sim, speed: number) => void;
  private readonly storage: CityStorage;

  constructor(
    post: (status: CityStatus) => void,
    install: (sim: Sim, speed: number) => void,
    storage: CityStorage = cityStorage,
  ) {
    this.post = post;
    this.install = install;
    this.storage = storage;
  }

  private status() {
    this.post({
      type: 'cityStatus',
      ready: this.ready,
      busy: this.operations > 0,
      dirty: this.revision !== this.savedRevision,
      savedAt: this.savedAt,
      error: this.error,
    });
  }

  /** All storage work is ordered, including replacing a city while a write is pending. */
  private run(action: () => Promise<void>, failure: string) {
    this.operations++;
    this.status();
    const next = this.queue.then(async () => {
      try {
        await action();
      } catch (error) {
        this.error = `${failure}: ${error instanceof Error ? error.message : String(error)}`;
      } finally {
        this.operations--;
        this.status();
      }
    });
    this.queue = next;
    return next;
  }

  start(options: Options) {
    if (this.options) return this.queue;
    this.options = options;
    const loading = this.run(async () => {
      const record = await this.storage.read();
      const sim = record ? Sim.load(record.bytes) : this.create();
      this.sim = sim;
      this.revision = record ? 0 : 1;
      this.savedRevision = 0;
      this.savedAt = record?.savedAt ?? null;
      this.ready = true;
      this.install(sim, record ? 0 : 1);
    }, 'Could not open your city. Your saved data has not been replaced');
    setInterval(() => {
      if (this.ready && !this.operations && this.revision !== this.savedRevision) void this.save();
    }, AUTOSAVE_MS);
    return loading;
  }

  private create() {
    const options = this.options;
    if (!options) throw new Error('The game is still starting.');
    const sim = options.grid
      ? new Sim(options.seed, options.cols, options.rows)
      : Sim.empty(options.seed, options.cols, options.rows);
    if (options.agents > 0) sim.spawn_agents(options.agents);
    return sim;
  }

  changed() {
    const wasSaved = this.revision === this.savedRevision;
    this.revision++;
    if (wasSaved) this.status();
  }

  /** Capture between complete ticks. Later edits remain dirty when this write commits. */
  private async persist(sim: Sim, replace = false) {
    const revision = this.revision;
    const bytes = sim.save();
    const savedAt = Date.now();
    await this.storage.write({ bytes, savedAt }, replace);
    this.savedRevision = revision;
    this.savedAt = savedAt;
    this.error = null;
  }

  save() {
    const sim = this.sim;
    if (!this.ready || !sim || this.operations) return this.queue;
    return this.run(() => this.persist(sim), 'Could not save your city. Try Save city again');
  }

  /** The current city's save bytes, for sharing. Changes nothing. */
  bytes(): Uint8Array {
    if (!this.ready || !this.sim) throw new Error('The city is still opening. Try again.');
    return this.sim.save();
  }

  /** The current city's command log, or null when it was not started fresh here. */
  commandLog(): string | null {
    return (this.ready && this.sim?.command_log()) || null;
  }

  /** A shared city replaces this one as an independent copy (ADR 0035), opened paused. The UI
   * confirms first. Bytes the sim rejects leave the current city and its save untouched. */
  open(bytes: Uint8Array) {
    if (!this.options) return this.queue;
    const wasReady = this.ready;
    this.ready = false;
    return this.run(async () => {
      let next: Sim;
      try {
        next = Sim.load(bytes);
      } catch (error) {
        this.ready = wasReady;
        const reason = error instanceof Error ? error.message : String(error);
        this.error = `Could not open that city (${reason}). Your current city is unchanged.`;
        return;
      }
      this.replace(next, 0);
      await this.persist(next, true);
    }, 'Opened the city but could not save it. Your previous stored city is unchanged; try Save city again');
  }

  private replace(next: Sim, speed: number) {
    this.sim?.free();
    this.sim = next;
    this.revision = 1;
    this.savedRevision = 0;
    this.savedAt = null;
    this.error = null;
    this.ready = true;
    this.install(next, speed);
  }

  /** The UI obtains confirmation before sending this request. */
  newCity() {
    if (!this.options) return this.queue;
    this.ready = false;
    return this.run(async () => {
      const next = this.create();
      this.replace(next, 1);
      await this.persist(next, true);
    }, 'Could not save the new city. Your previous stored city is unchanged; try Save city again');
  }
}
