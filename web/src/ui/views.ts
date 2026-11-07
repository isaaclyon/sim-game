// The Views list (M4.6): one ground overlay at a time, plus traffic colours and power, water
// and fire markers as separate switches, combining as before. Display only. P, T and O work as they always have.
import { h, press, tooltip } from './dom';
import type { Popovers } from './popover';

/** `plain` is the everyday look; the rest are the overlays P cycles through. */
export const POLLUTION_VIEWS = ['plain', 'air', 'ground', 'noise', 'land'] as const;
export type PollutionView = (typeof POLLUTION_VIEWS)[number];

const OVERLAY_NAMES: Record<PollutionView, string> = {
  plain: 'None',
  air: 'Air pollution',
  ground: 'Ground pollution',
  noise: 'Noise',
  land: 'Land value',
};

export function nextPollutionView(view: PollutionView): PollutionView {
  return POLLUTION_VIEWS[(POLLUTION_VIEWS.indexOf(view) + 1) % POLLUTION_VIEWS.length];
}

/** A service whose markers can show: power grids (ADR 0041), water networks (ADR 0052) or
 * fire coverage (ADR 0053). */
export type Service = 'power' | 'water' | 'fire';

const MARKERS: Record<Service, { id: string; label: string; tip: string; key?: string }> = {
  power: {
    id: 'power-view',
    label: 'Power markers',
    tip: "Each building in its grid's colour; red has no power.",
    key: 'O',
  },
  water: {
    id: 'water-view',
    label: 'Water markers',
    tip: "Pumps, towers and dense buildings in their network's colour; red is dry.",
  },
  fire: {
    id: 'fire-view',
    label: 'Fire coverage',
    tip: 'Roads and buildings a fire station reaches in time.',
  },
};

/** What the views draw on; `Renderer` implements it. */
export interface ViewDrawing {
  toggleTrafficView(): boolean;
  setPollutionView(view: PollutionView): void;
  setPowerView(on: boolean): void;
  setWaterView(on: boolean): void;
  setFireView(on: boolean): void;
}

export class Views {
  /** Which service markers are on. */
  readonly markers: Record<Service, boolean> = { power: false, water: false, fire: false };
  /** Called when the player switches a service's markers, by key or button. */
  onPlayerMarkers: ((service: Service) => void) | null = null;
  private overlay: PollutionView = 'plain';
  private traffic = false;
  private readonly draw: ViewDrawing;
  private readonly overlayButtons: HTMLElement[];
  private readonly trafficButton: HTMLElement;
  private readonly markerButtons: Record<Service, HTMLElement>;
  private readonly button: HTMLElement;

  /** `button` is the Views button in the category bar; the list opens above it. */
  constructor(button: HTMLElement, draw: ViewDrawing, popovers: Popovers) {
    this.draw = draw;
    this.button = button;
    this.overlayButtons = POLLUTION_VIEWS.map((view) => {
      const b = h(
        'button',
        { type: 'button', id: `view-${view}`, 'aria-pressed': String(view === 'plain') },
        OVERLAY_NAMES[view],
      );
      b.onclick = () => this.setOverlay(view);
      return b;
    });
    this.trafficButton = h(
      'button',
      {
        type: 'button',
        id: 'traffic',
        'aria-pressed': 'false',
        'aria-keyshortcuts': 'T',
        title: tooltip('Traffic colours', 'Red stopped, green at the limit.', 'T'),
      },
      'Traffic colours',
    );
    this.trafficButton.onclick = () => this.toggleTraffic();
    const marker = (service: Service) => {
      const m = MARKERS[service];
      const b = h(
        'button',
        {
          type: 'button',
          id: m.id,
          'aria-pressed': 'false',
          ...(m.key ? { 'aria-keyshortcuts': m.key } : {}),
          title: tooltip(m.label, m.tip, m.key),
        },
        m.label,
      );
      b.onclick = () => this.toggleMarkers(service);
      return b;
    };
    this.markerButtons = { power: marker('power'), water: marker('water'), fire: marker('fire') };
    const panel = h(
      'section',
      { id: 'views-panel', class: 'glass panel views', 'aria-label': 'Views', hidden: true },
      h('h2', {}, 'Views'),
      h(
        'div',
        {
          class: 'choices',
          role: 'group',
          'aria-label': 'Ground overlay',
          title: tooltip('Ground overlay', 'One at a time.', 'P cycles'),
        },
        ...this.overlayButtons,
      ),
      h('h2', {}, 'Also show'),
      h(
        'div',
        { class: 'choices' },
        this.trafficButton,
        this.markerButtons.power,
        this.markerButtons.water,
        this.markerButtons.fire,
      ),
    );
    document.body.append(panel);
    button.setAttribute('aria-controls', 'views-panel');
    popovers.bind(button, panel, true);
  }

  setOverlay(view: PollutionView) {
    this.overlay = view;
    this.draw.setPollutionView(view);
    POLLUTION_VIEWS.forEach((v, i) => {
      press(this.overlayButtons[i], v === view);
    });
    this.mark();
  }

  toggleTraffic() {
    this.traffic = this.draw.toggleTrafficView();
    press(this.trafficButton, this.traffic);
    this.mark();
  }

  /** Shows or hides a service's markers without counting as the player's choice. */
  setMarkers(service: Service, on: boolean) {
    this.markers[service] = on;
    if (service === 'power') this.draw.setPowerView(on);
    else if (service === 'water') this.draw.setWaterView(on);
    else this.draw.setFireView(on);
    press(this.markerButtons[service], on);
    this.mark();
  }

  toggleMarkers(service: Service) {
    this.setMarkers(service, !this.markers[service]);
    this.onPlayerMarkers?.(service);
  }

  /** P cycles the ground overlays, T and O switch traffic colours and power markers. */
  key(key: string): boolean {
    const k = key.toLowerCase();
    if (k === 'p') this.setOverlay(nextPollutionView(this.overlay));
    else if (k === 't') this.toggleTraffic();
    else if (k === 'o') this.toggleMarkers('power');
    else return false;
    return true;
  }

  /** The Views button reads as on while any view shows. */
  private mark() {
    const { power, water, fire } = this.markers;
    const any = power || water || fire;
    this.button.classList.toggle('on', this.overlay !== 'plain' || this.traffic || any);
  }
}
