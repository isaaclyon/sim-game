import { cityFromLink, cityLink } from './city-share';
import type { CityStatus, FromWorker, ToWorker } from './protocol';

type Lookup = (id: string) => HTMLElement;
type Reply = Extract<FromWorker, { type: 'cityFile' | 'commandLog' }>;
const OPEN_PROMPT =
  'Open this city? It replaces your current city and its saved progress. This cannot be undone.';

function download(name: string, data: BlobPart, type: string) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([data], { type }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 60_000);
}

/** Save and share controls report worker acknowledgements, never an optimistic success. */
export class CityControls {
  private readonly send: (message: ToWorker) => void;
  private readonly save: HTMLButtonElement;
  private readonly newCity: HTMLButtonElement;
  private readonly share: HTMLButtonElement[];
  private readonly status: HTMLElement;
  private readonly shareStatus: HTMLElement;
  private readonly linkField: HTMLInputElement;
  /** Tracks an open request until the worker reports it busy and then settled. */
  private opening: 'sent' | 'busy' | null = null;
  private last: CityStatus | null = null;
  private exporting: 'download' | 'link' | null = null;
  /** A city from the page's link, opened once the startup city has settled. */
  private linked: Promise<Uint8Array | null> | null = null;

  constructor(send: (message: ToWorker) => void, byId: Lookup, dev = false) {
    this.send = send;
    this.save = byId('save-city') as HTMLButtonElement;
    this.newCity = byId('new-city') as HTMLButtonElement;
    this.status = byId('save-status');
    this.shareStatus = byId('share-status');
    this.linkField = byId('share-url') as HTMLInputElement;
    const fileInput = byId('open-city-file') as HTMLInputElement;
    const [downloadCity, copyLink, openFile, log] = [
      'download-city',
      'copy-link',
      'open-city',
      'command-log',
    ].map((id) => byId(id) as HTMLButtonElement);
    this.share = [downloadCity, copyLink, openFile];
    log.hidden = !dev;
    this.save.disabled = this.newCity.disabled = true;
    for (const b of this.share) b.disabled = true;
    this.save.onclick = () => {
      if (!this.save.disabled) send({ type: 'saveCity' });
    };
    this.newCity.onclick = () => {
      if (
        !this.newCity.disabled &&
        confirm(
          'Start a new city? This replaces your current city and its saved progress. This cannot be undone.',
        )
      )
        send({ type: 'newCity' });
    };
    downloadCity.onclick = () => this.export('download');
    copyLink.onclick = () => this.export('link');
    openFile.onclick = () => fileInput.click();
    fileInput.onchange = async () => {
      const file = fileInput.files?.[0];
      fileInput.value = '';
      if (file && confirm(OPEN_PROMPT)) this.open(new Uint8Array(await file.arrayBuffer()));
    };
    log.onclick = () => send({ type: 'commandLog' });
    this.readLink();
    // A link pasted into a tab that already runs the game only changes the fragment.
    addEventListener('hashchange', () => {
      this.readLink();
      if (this.last && !this.last.busy) void this.openLinked(this.last, true);
    });
  }

  private readLink() {
    if (!location.hash) return;
    this.linked = cityFromLink(location.hash);
    // Reported once startup settles; until then the rejection is expected, not unhandled.
    this.linked.catch(() => {});
    // A reload must resume the local copy, not reopen the original link.
    history.replaceState(null, '', location.pathname + location.search);
  }

  private note(text: string, error = false) {
    this.shareStatus.textContent = text;
    this.shareStatus.dataset.error = String(error);
  }

  private export(kind: 'download' | 'link') {
    this.exporting = kind;
    this.linkField.hidden = true;
    this.note(kind === 'link' ? 'Making a link…' : 'Preparing the city file…');
    this.send({ type: 'exportCity' });
  }

  private open(bytes: Uint8Array) {
    this.note('Opening the shared city…');
    this.opening = 'sent';
    this.send({ type: 'openCity', bytes });
  }

  /** Opens a linked city once startup settles. Only a first visit's untouched fresh city is
   * replaced without asking; a running game may hold unsaved edits. */
  private async openLinked(status: CityStatus, ask = false) {
    const linked = this.linked;
    this.linked = null;
    if (!linked) return;
    try {
      const bytes = await linked;
      if (!bytes) return;
      const fresh = !ask && status.ready && status.savedAt === null && !status.error;
      if (fresh || confirm(OPEN_PROMPT)) this.open(bytes);
      else this.note('Kept your city. The shared city was not opened.');
    } catch (error) {
      this.note(error instanceof Error ? error.message : String(error), true);
    }
  }

  receive(message: CityStatus) {
    this.last = message;
    if (this.linked && !message.busy) void this.openLinked(message);
    if (this.opening && message.busy) this.opening = 'busy';
    else if (this.opening === 'busy') {
      this.opening = null;
      // A refused file leaves the city as it was; the reason shows in the save status.
      this.note(message.error ? '' : 'Opened, paused. This copy is yours; changes stay here.');
    }
    this.save.disabled = !message.ready || message.busy;
    this.newCity.disabled = message.busy;
    this.share[0].disabled = this.share[1].disabled = !message.ready || message.busy;
    this.share[2].disabled = message.busy;
    this.status.dataset.error = String(!!message.error);
    const saved =
      message.savedAt === null ? '' : `Saved at ${new Date(message.savedAt).toLocaleTimeString()}`;
    this.status.textContent = message.busy
      ? message.ready
        ? 'Saving city…'
        : 'Opening city…'
      : (message.error ??
        (message.dirty
          ? `Unsaved changes${saved ? ` · ${saved}` : ''}`
          : saved || 'No saved city yet.'));
  }

  async receiveFile(message: Reply) {
    if (message.type === 'commandLog') {
      if (message.text === null)
        return this.note('No command log: this city was opened, not started here.', true);
      return download(`city-${Date.now()}.cmdlog`, message.text, 'text/plain');
    }
    const kind = this.exporting;
    this.exporting = null;
    if (!kind) return;
    if (!message.bytes) return this.note(message.error ?? 'Could not export the city.', true);
    if (kind === 'download') {
      download(
        `city-${Date.now()}.city`,
        message.bytes as Uint8Array<ArrayBuffer>,
        'application/octet-stream',
      );
      return this.note('Downloaded. Send the file; friends open it with Open file.');
    }
    const link = await cityLink(message.bytes, location.href);
    if (!link) return this.note('This city is too big for a link. Use Download instead.', true);
    this.linkField.value = link;
    this.linkField.hidden = false;
    try {
      await navigator.clipboard.writeText(link);
      this.note('Link copied. Anyone who opens it gets their own copy of this city.');
    } catch {
      this.linkField.select();
      this.note('Copy the link below. Anyone who opens it gets their own copy of this city.');
    }
  }
}
