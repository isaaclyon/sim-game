import type { CityStatus, ToWorker } from './protocol';

/** Save controls report worker acknowledgements, never an optimistic success. */
export class CityControls {
  private readonly save: HTMLButtonElement;
  private readonly newCity: HTMLButtonElement;
  private readonly status: HTMLElement;

  constructor(
    send: (message: ToWorker) => void,
    save: HTMLButtonElement,
    newCity: HTMLButtonElement,
    status: HTMLElement,
  ) {
    this.save = save;
    this.newCity = newCity;
    this.status = status;
    save.disabled = newCity.disabled = true;
    save.onclick = () => {
      if (!save.disabled) send({ type: 'saveCity' });
    };
    newCity.onclick = () => {
      if (
        !newCity.disabled &&
        confirm(
          'Start a new city? This replaces your current city and its saved progress. This cannot be undone.',
        )
      )
        send({ type: 'newCity' });
    };
  }

  receive(message: CityStatus) {
    this.save.disabled = !message.ready || message.busy;
    this.newCity.disabled = message.busy;
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
}
