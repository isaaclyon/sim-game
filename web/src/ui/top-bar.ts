// The top bar: clock, speed, population, money, power, water, fire, R/C/I demand and the save
// status.
// Clicking a figure opens its details; a figure turns red when something is wrong.
import { type CityStatus, FIRE_STRIDE, type Stats } from '../protocol';
import {
  clockText,
  demandFill,
  demandLabel,
  dollars,
  fireFigures,
  fmt,
  perHour,
  populationRows,
  powerAdvice,
  powerFigures,
  saveBadge,
  waterAdvice,
  waterFigures,
} from './figures';
import type { Popovers } from './popover';

type Lookup = (id: string) => HTMLElement;

/** What the figures' panels can do outside the top bar. */
export interface TopBarActions {
  setSpeed(speed: number): void;
  /** Opens a build menu category, focusing its first item when asked from the keyboard. */
  openCategory(id: string, focus: boolean): void;
  /** Asks the sim to turn this city's fires on or off. */
  setFires(on: boolean): void;
  /** Moves the camera to a sim point. */
  showPoint(x: number, y: number): void;
}
const SPEEDS = [0, 1, 3, 10];
const USES = ['residential', 'commercial', 'industrial'] as const;

/** Writes only on change, so a frame that changes nothing touches no text. */
function put(el: HTMLElement, text: string) {
  if (el.textContent !== text) el.textContent = text;
}
function show(el: HTMLElement, on: boolean) {
  if (el.hidden === on) el.hidden = !on;
}
function mark(el: HTMLElement, name: string, on: boolean) {
  el.classList.toggle(name, on);
}

export class TopBar {
  private readonly byId: Lookup;
  private readonly speeds: HTMLElement[];
  private city: CityStatus | null = null;
  /** The sim's burning buildings, `[x, y, height, truck]` each, and the next one to show. */
  private fires: Float32Array = new Float32Array();
  private nextFire = 0;
  private firesOn = true;

  constructor(byId: Lookup, popovers: Popovers, actions: TopBarActions) {
    this.byId = byId;
    this.speeds = SPEEDS.map((s) => byId(`speed-${s}`));
    this.speeds.forEach((b, i) => {
      b.onclick = () => actions.setSpeed(SPEEDS[i]);
    });
    for (const figure of ['population', 'money', 'power', 'water', 'fire'])
      popovers.bind(byId(figure), byId(`${figure}-panel`));
    popovers.bind(byId('menu-button'), byId('menu'), false, byId('save-badge'));
    for (const [button, category] of [
      ['open-power', 'power'],
      ['open-water', 'water'],
      ['open-services', 'services'],
    ])
      byId(button).onclick = (e?: MouseEvent) => {
        popovers.close();
        actions.openCategory(category, e?.detail === 0);
      };
    byId('fire-setting').onclick = () => actions.setFires(!this.firesOn);
    byId('show-fire').onclick = () => {
      const at = this.fireToShow();
      if (at) actions.showPoint(at[0], at[1]);
    };
  }

  /** The `fires` list of the latest fire message (see protocol.ts). */
  setFires(fires: Float32Array) {
    this.fires = fires;
  }

  showSpeed(speed: number) {
    this.speeds.forEach((b, i) => {
      b.setAttribute('aria-pressed', String(SPEEDS[i] === speed));
    });
  }

  setCity(status: CityStatus) {
    this.city = status;
  }

  /** Redraws every figure from the latest stats; `now` is the wall clock for "saved 2m ago". */
  draw(s: Stats | undefined, now: number) {
    const $ = this.byId;
    const badge = saveBadge(this.city, now);
    const saveEl = $('save-badge');
    put(saveEl, badge.text);
    saveEl.dataset.error = String(badge.error);
    if (!s) return;
    put($('clock'), clockText(s));
    put($('population-value'), `Pop ${fmt(s.population)}`);
    for (const [id, text] of Object.entries(populationRows(s))) put($(id), text);
    this.drawMoney(s);
    this.drawPower(s);
    this.drawWater(s);
    this.drawFire(s);
    this.drawDemand(s);
  }

  private drawMoney(s: Stats) {
    const $ = this.byId;
    const net = s.taxes + s.grant - s.upkeep - s.utilities;
    const debt = s.balance < 0;
    put($('money-value'), dollars(s.balance));
    put($('money-rate'), perHour(net));
    mark($('money-rate'), 'losing', net < 0);
    mark($('money'), 'warning', debt);
    put($('money-title'), debt ? 'In debt' : 'Money');
    mark($('money-title'), 'problem', debt);
    put($('money-balance'), dollars(s.balance));
    put($('money-taxes'), perHour(s.taxes));
    put($('money-grant'), perHour(s.grant));
    put($('money-upkeep'), perHour(-s.upkeep));
    put($('money-utilities'), perHour(-s.utilities));
    put($('money-net'), perHour(net));
    show($('money-debt'), debt);
    $('money').setAttribute(
      'aria-label',
      `Money ${dollars(s.balance)}, ${perHour(net)}${debt ? ', in debt' : ''}`,
    );
  }

  private drawPower(s: Stats) {
    const $ = this.byId;
    const p = s.power;
    const { available, short, warning, text } = powerFigures(p);
    const waitingText = `${fmt(p.waiting)} ${p.waiting === 1 ? 'lot' : 'lots'} waiting for power`;
    put($('power-value'), text);
    mark($('power'), 'warning', warning);
    const why = short ? ', not enough power' : warning ? `, ${waitingText}` : '';
    $('power').setAttribute('aria-label', `Power: ${text} of supply needed${why}`);
    put($('power-title'), short ? 'Not enough power' : warning ? 'More power needed' : 'Power');
    mark($('power-title'), 'problem', warning);
    put($('power-needed'), fmt(p.demand));
    put($('power-available'), `${fmt(available)} (starter ${fmt(p.starter)})`);
    const unpowered = $('power-unpowered');
    put(
      unpowered,
      `${fmt(p.unpowered)} ${p.unpowered === 1 ? 'building' : 'buildings'} without power`,
    );
    show(unpowered, p.unpowered > 0);
    const waiting = $('power-waiting');
    put(waiting, waitingText);
    show(waiting, p.waiting > 0);
    put($('power-advice'), powerAdvice(p));
    // The bar spans the larger of need and supply; the mark is the supply.
    const scale = Math.max(p.demand, available, 1);
    const width = (n: number) => `${((100 * n) / scale).toFixed(1)}%`;
    $('power-meter-fill').style.width = width(Math.min(p.demand, available));
    $('power-meter-over').style.left = width(available);
    $('power-meter-over').style.width = width(Math.max(p.demand - available, 0));
    $('power-meter-mark').style.left = width(available);
  }

  private drawWater(s: Stats) {
    const $ = this.byId;
    const w = s.water;
    const { short, text } = waterFigures(w);
    put($('water-value'), text);
    mark($('water'), 'warning', short);
    $('water').setAttribute(
      'aria-label',
      `Water: ${text} of pump output needed${short ? ', buildings are dry' : ''}`,
    );
    put($('water-title'), short ? 'Not enough water' : 'Water');
    mark($('water-title'), 'problem', short);
    put($('water-needed'), fmt(w.demand));
    put($('water-pumped'), fmt(w.supply));
    put($('water-stored'), `${fmt(w.stored)} of ${fmt(w.capacity)}`);
    const dry = $('water-dry');
    put(dry, `${fmt(w.dry)} dense ${w.dry === 1 ? 'building' : 'buildings'} without water`);
    show(dry, w.dry > 0);
    const waiting = $('water-waiting');
    put(waiting, `${fmt(w.waiting)} ${w.waiting === 1 ? 'lot' : 'lots'} waiting for water`);
    show(waiting, w.waiting > 0);
    put($('water-advice'), waterAdvice(w));
  }

  private drawFire(s: Stats) {
    const $ = this.byId;
    const f = s.fire;
    const { alarm, near, text, advice } = fireFigures(f, s.population);
    this.firesOn = f.enabled;
    put($('fire-value'), text);
    mark($('fire'), 'warning', alarm);
    mark($('fire'), 'caution', near);
    $('fire').setAttribute(
      'aria-label',
      `Fire: ${text}${alarm ? ', a fire has no truck' : near ? ', fires start soon' : ''}`,
    );
    put($('fire-title'), alarm ? 'Fire with no truck' : 'Fire');
    mark($('fire-title'), 'problem', alarm);
    put($('fire-stations'), fmt(f.stations));
    put($('fire-trucks'), `${fmt(f.freeTrucks)} of ${fmt(f.trucks)} free`);
    put($('fire-burning'), fmt(f.burning));
    put($('fire-covered'), fmt(f.covered));
    put($('fire-saved'), fmt(f.saved));
    put($('fire-destroyed'), fmt(f.destroyed));
    put($('fire-advice'), advice);
    show($('show-fire'), f.burning > 0);
    const setting = $('fire-setting');
    put(setting, f.enabled ? 'Turn fires off' : 'Turn fires on');
    setting.setAttribute('aria-pressed', String(!f.enabled));
  }

  /** Each press shows the next burning building, those with no truck first. */
  private fireToShow(): [number, number] | null {
    const f = this.fires;
    const n = f.length / FIRE_STRIDE;
    if (n === 0) return null;
    const order = [...Array(n).keys()].sort(
      (a, b) => f[a * FIRE_STRIDE + 3] - f[b * FIRE_STRIDE + 3],
    );
    const i = order[this.nextFire++ % n] * FIRE_STRIDE;
    return [f[i], f[i + 1]];
  }

  /** ADR 0025: a bar grows right of the middle for demand and left of it for a surplus. */
  private drawDemand(s: Stats) {
    const $ = this.byId;
    for (const use of USES) {
      const f = demandFill(s.demand[use]);
      const style = $(`demand-${use}`).style;
      style.width = `${(Math.abs(f) * 50).toFixed(1)}%`;
      style.left = f >= 0 ? '50%' : `${(50 - Math.abs(f) * 50).toFixed(1)}%`;
      mark($(`demand-${use}`), 'surplus', f < 0);
    }
    const label = demandLabel(s.demand);
    const root = $('demand');
    if (root.getAttribute('aria-label') !== label) {
      root.setAttribute('aria-label', label);
      root.title = label;
    }
  }
}
