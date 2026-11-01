// Citizen inspector (ADR 0022): which car is selected and whether the camera follows it. The
// sim owns every fact shown; this only picks a visible car and prints what the worker sends.

import type { Citizen, Place, ToWorker } from './protocol';

/** Words for a building by its original use, indexed by `Zone`. */
const BUILDING_WORDS = ['building', 'house', 'apartments', 'shop', 'store', 'factory', 'factory'];
const label = (p: Place) => `${BUILDING_WORDS[p.zone] ?? 'building'} #${p.building}`;

/** The panel's text for `c`: name, then home, job and what they are doing now. */
export function describe(c: Citizen): string {
  if (!c.resident) return `${c.name}\nnow    left town`;
  const there = (p: Place | null) => (p ? ` at ${label(p)}` : '');
  // Indexed by `Activity`: home, work, shop.
  const trip = ['home', `to work${there(c.destination)}`, `to shop${there(c.destination)}`];
  const stay = [
    'at home',
    `at work${c.location ? `, ${label(c.location)}` : ''}`,
    `shopping${there(c.location)}`,
  ];
  const now = !c.destination
    ? stay[c.activity]
    : `${c.driving ? 'driving' : 'waiting to drive'} ${trip[c.activity]}`;
  return [
    c.name,
    `home   ${c.home ? label(c.home) : 'none'}`,
    `job    ${c.job ? label(c.job) : 'unemployed'}`,
    `now    ${now}`,
  ].join('\n');
}

interface Panel {
  hidden: boolean | string;
}
interface Text {
  textContent: string | null;
}
interface Button {
  disabled: boolean;
  setAttribute(key: string, value: string): void;
  onclick: ((event: never) => unknown) | null;
}

/** Hooks into the renderer the inspector needs: highlight, follow and zoom target. */
export interface InspectorView {
  /** Camera follows car `agent`; false while it is not drawn. */
  follow(agent: number, seconds: number): boolean;
  /** Zoom toward the followed car instead of the cursor while following. */
  setFollowing(on: boolean): void;
}

export class Inspector {
  /** Bumped on every selection change, so replies to an older one are dropped. */
  private selection = 0;
  /** The selected car slot, which belongs to one citizen for good (ADR 0021). */
  private agent: number | null = null;
  private citizen: Citizen | null = null;
  private following = false;

  private readonly send: (msg: ToWorker) => void;
  private readonly view: InspectorView;
  private readonly panel: Panel;
  private readonly text: Text;
  private readonly followButton: Button;

  constructor(
    send: (msg: ToWorker) => void,
    view: InspectorView,
    panel: Panel,
    text: Text,
    followButton: Button,
    close: Button,
  ) {
    this.send = send;
    this.view = view;
    this.panel = panel;
    this.text = text;
    this.followButton = followButton;
    panel.hidden = true;
    followButton.onclick = () => this.toggleFollow();
    close.onclick = () => this.select(null);
  }

  /** The car to highlight, or null. */
  get selected(): number | null {
    return this.agent;
  }

  get isFollowing(): boolean {
    return this.following;
  }

  /** Selects the resident of car `agent`, or clears with null. Reselecting is a no-op. */
  select(agent: number | null) {
    if (agent === this.agent) return;
    this.selection++;
    this.agent = agent;
    this.citizen = null;
    this.setFollow(false);
    this.panel.hidden = true;
    this.send({ type: 'inspect', agent, selection: this.selection });
  }

  /** A `citizen` reply from the worker. */
  receive(selection: number, citizen: Citizen | null) {
    if (selection !== this.selection || this.agent === null) return;
    if (!citizen) {
      this.agent = null;
      this.citizen = null;
      this.setFollow(false);
      this.panel.hidden = true;
      return;
    }
    this.citizen = citizen;
    if (!citizen.resident) this.setFollow(false);
    const text = describe(citizen);
    if (this.text.textContent !== text) this.text.textContent = text;
    this.followButton.disabled = !citizen.resident;
    this.panel.hidden = false;
  }

  toggleFollow() {
    if (this.citizen?.resident) this.setFollow(!this.following);
  }

  setFollow(on: boolean) {
    if (on === this.following) return;
    this.following = on;
    this.view.setFollowing(on);
    this.followButton.setAttribute('aria-pressed', String(on));
  }

  /** Moves the camera with the followed car; it holds still while the car is hidden. */
  update(seconds: number) {
    if (this.following && this.agent !== null) this.view.follow(this.agent, seconds);
  }
}
