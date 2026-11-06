// The Views list (M4.6): one ground overlay at a time, plus traffic colours and power markers
// as separate switches, combining as before. Display only. P, T and O work as they always have.
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

/** What the views draw on; `Renderer` implements it. */
export interface ViewDrawing {
  toggleTrafficView(): boolean;
  setPollutionView(view: PollutionView): void;
  setPowerView(on: boolean): void;
}

export class Views {
  /** The power markers are on. */
  power = false;
  /** Called when the player switches the power markers, by key or button. */
  onPlayerPower: (() => void) | null = null;
  private overlay: PollutionView = 'plain';
  private traffic = false;
  private readonly draw: ViewDrawing;
  private readonly overlayButtons: HTMLElement[];
  private readonly trafficButton: HTMLElement;
  private readonly powerButton: HTMLElement;
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
    this.powerButton = h(
      'button',
      {
        type: 'button',
        id: 'power-view',
        'aria-pressed': 'false',
        'aria-keyshortcuts': 'O',
        title: tooltip(
          'Power markers',
          "Each building in its grid's colour; red has no power.",
          'O',
        ),
      },
      'Power markers',
    );
    this.powerButton.onclick = () => this.togglePower();
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
      h('div', { class: 'choices' }, this.trafficButton, this.powerButton),
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

  /** Shows or hides the power markers without counting as the player's choice. */
  setPower(on: boolean) {
    this.power = on;
    this.draw.setPowerView(on);
    press(this.powerButton, on);
    this.mark();
  }

  togglePower() {
    this.setPower(!this.power);
    this.onPlayerPower?.();
  }

  /** P cycles the ground overlays, T and O switch traffic colours and power markers. */
  key(key: string): boolean {
    const k = key.toLowerCase();
    if (k === 'p') this.setOverlay(nextPollutionView(this.overlay));
    else if (k === 't') this.toggleTraffic();
    else if (k === 'o') this.togglePower();
    else return false;
    return true;
  }

  /** The Views button reads as on while any view shows. */
  private mark() {
    this.button.classList.toggle('on', this.overlay !== 'plain' || this.traffic || this.power);
  }
}
