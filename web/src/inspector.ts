// Citizen and building inspector: one selected subject, with sim-owned live counts and facts,
// shown as a card with a title and one small icon per line (M4.6).

import type { Building, Citizen, Place, ToWorker } from './protocol';
import { COMPLAINT_WORDS, moodText } from './ui/figures';
import { type IconName, iconSvg } from './ui/icons';

/** Words for a building by its original use, indexed by `Zone`. */
const BUILDING_WORDS = [
  'building',
  'house',
  'apartments',
  'shop',
  'store',
  'factory',
  'factory',
  'primary school',
  'high school',
];
const label = (p: Place) => `${BUILDING_WORDS[p.zone] ?? 'building'} #${p.building}`;
const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export interface CardLine {
  icon: IconName;
  text: string;
}
/** What the inspector card shows: a title, then one line per fact. */
export interface Card {
  title: string;
  lines: CardLine[];
}

/** The card for `c`: name, then home, job, mood and complaints, and what they are doing now. */
export function describe(c: Citizen): Card {
  if (!c.resident) return { title: c.name, lines: [{ icon: 'leave', text: 'Left town' }] };
  const there = (p: Place | null) => (p ? ` at ${label(p)}` : '');
  // Indexed by `Activity`: home, work, shop, school.
  const trip = [
    'home',
    `to work${there(c.destination)}`,
    `to shop${there(c.destination)}`,
    `to school${there(c.destination)}`,
  ];
  const stay: CardLine[] = [
    { icon: 'home', text: 'At home' },
    { icon: 'work', text: `At work${c.location ? `, ${label(c.location)}` : ''}` },
    { icon: 'shop', text: `Shopping${there(c.location)}` },
    { icon: 'work', text: `At school${c.location ? `, ${label(c.location)}` : ''}` },
  ];
  const how: [IconName, string] = c.onFoot
    ? ['walk', 'Walking']
    : c.driving
      ? ['car', 'Driving']
      : ['clock', 'Waiting to drive'];
  const now: CardLine = !c.destination
    ? stay[c.activity]
    : { icon: how[0], text: `${how[1]} ${trip[c.activity]}` };
  return {
    title: c.name,
    lines: [
      { icon: 'home', text: `Home: ${c.home ? label(c.home) : 'none'}` },
      c.child
        ? { icon: 'people', text: `Age ${c.age}, a child: lives at home` }
        : { icon: 'work', text: c.job ? `Job: ${label(c.job)}${commute(c)}` : 'Job: unemployed' },
      ...(c.child ? [] : [{ icon: 'people', text: `Age ${c.age}${grewUp(c)}` } as const]),
      { icon: 'mood', text: `Mood: ${moodText(c.mood)}` },
      ...(c.complaints.length
        ? [
            {
              icon: 'warning',
              text: `Complaints: ${c.complaints.map((k) => COMPLAINT_WORDS[k]).join(', ')}`,
            } as const,
          ]
        : []),
      now,
    ],
  };
}

/** How long the last trip to work took, once there has been one (ADR 0058). */
function commute(c: Citizen): string {
  return c.commute > 0 ? `, ${c.commute} min commute` : '';
}

/** Where a grown child of the town stands on finding a home (ADR 0054). */
function grewUp(c: Citizen): string {
  if (!c.grewUp) return '';
  return `, grew up here: ${c.wantsHome ? 'looking for' : 'has'} a home of their own`;
}

/** Counts describe residency or assigned jobs, never physical attendance. */
export function describeBuilding(b: Building): Card {
  const residential = b.zone === 1 || b.zone === 2;
  return {
    title: capital(label(b)),
    lines: [
      { icon: 'layers', text: `${b.zone % 2 ? 'Low' : 'High'} density` },
      ...(b.closing
        ? [{ icon: 'warning', text: 'Closing: not accepting new residents or workers' } as const]
        : []),
      ...(b.watered
        ? []
        : [
            {
              icon: 'warning',
              text: 'No water: no one moves in, and people leave after a day',
            } as const,
          ]),
      ...(residential
        ? ([
            { icon: 'building', text: `Resident capacity: ${b.capacity}` },
            { icon: 'people', text: `Residents: ${b.occupied}` },
          ] as const)
        : ([
            { icon: 'work', text: `Total jobs: ${b.capacity}` },
            { icon: 'people', text: `Filled jobs: ${b.occupied}` },
            { icon: 'open', text: `Unfilled jobs: ${b.capacity - b.occupied}` },
          ] as const)),
    ],
  };
}

/** The card as plain text, one fact per line; what a screen reader reads. */
export const cardText = (card: Card) => [card.title, ...card.lines.map((l) => l.text)].join('\n');

interface Button {
  hidden: boolean | string;
  disabled: boolean;
  setAttribute(key: string, value: string): void;
  onclick: ((event: never) => unknown) | null;
}

/** The card's parts in index.html. */
export interface CardElements {
  panel: HTMLElement;
  title: HTMLElement;
  /** A list; each fact becomes one item with its icon. */
  lines: HTMLElement;
  follow: Button;
  close: Button;
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
  private building: number | null = null;
  private following = false;
  /** The card's text as last drawn, so an unchanged reply touches nothing. */
  private drawn = '';

  private readonly send: (msg: ToWorker) => void;
  private readonly view: InspectorView;
  private readonly el: CardElements;

  constructor(send: (msg: ToWorker) => void, view: InspectorView, el: CardElements) {
    this.send = send;
    this.view = view;
    this.el = el;
    el.panel.hidden = true;
    el.follow.onclick = () => this.toggleFollow();
    el.close.onclick = () => this.select(null);
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
    if (agent === this.agent && this.building === null) return;
    this.selection++;
    this.building = null;
    this.el.follow.hidden = false;
    this.agent = agent;
    this.citizen = null;
    this.setFollow(false);
    this.hide();
    this.send({ type: 'inspect', agent, selection: this.selection });
  }

  /** Selects a stable building ID. Citizen and building inspection share one card. */
  selectBuilding(building: number) {
    if (building === this.building) return;
    this.selection++;
    this.building = building;
    this.agent = null;
    this.citizen = null;
    this.setFollow(false);
    this.el.follow.hidden = true;
    this.hide();
    this.send({ type: 'inspectBuilding', building, selection: this.selection });
  }

  receiveBuilding(selection: number, building: Building | null) {
    if (selection !== this.selection || this.building === null) return;
    if (!building) return this.select(null);
    this.draw(describeBuilding(building));
  }

  /** A `citizen` reply from the worker. */
  receive(selection: number, citizen: Citizen | null) {
    if (selection !== this.selection || this.agent === null) return;
    if (!citizen) {
      this.agent = null;
      this.citizen = null;
      this.setFollow(false);
      this.hide();
      return;
    }
    this.citizen = citizen;
    if (!citizen.resident) this.setFollow(false);
    this.el.follow.disabled = !citizen.resident;
    this.draw(describe(citizen));
  }

  toggleFollow() {
    if (this.citizen?.resident) this.setFollow(!this.following);
  }

  setFollow(on: boolean) {
    if (on === this.following) return;
    this.following = on;
    this.view.setFollowing(on);
    this.el.follow.setAttribute('aria-pressed', String(on));
  }

  /** Moves the camera with the followed car; it holds still while the car is hidden. */
  update(seconds: number) {
    if (this.following && this.agent !== null) this.view.follow(this.agent, seconds);
  }

  /** Hides and empties the card, so the next subject's card is read in full. */
  private hide() {
    this.el.panel.hidden = true;
    this.el.title.textContent = '';
    this.el.lines.replaceChildren();
    this.drawn = '';
  }

  private draw(card: Card) {
    this.el.panel.hidden = false;
    const text = cardText(card);
    if (text === this.drawn) return;
    this.drawn = text;
    // Only what changed is rewritten, so a screen reader announces just that.
    if (this.el.title.textContent !== card.title) this.el.title.textContent = card.title;
    const list = this.el.lines;
    card.lines.forEach((line, i) => {
      let li = list.children[i] as HTMLElement | undefined;
      if (!li) {
        li = document.createElement('li');
        list.append(li);
      }
      let span = li.querySelector('span');
      if (!span || li.dataset.icon !== line.icon) {
        span ??= document.createElement('span');
        li.dataset.icon = line.icon;
        li.innerHTML = iconSvg(line.icon);
        li.append(span);
      }
      if (span.textContent !== line.text) span.textContent = line.text;
    });
    while (list.children.length > card.lines.length) list.children[card.lines.length].remove();
  }
}
