// The top bar: clock, speed, population, money, power, R/C/I demand and the save status.
// Clicking a figure opens its details; a figure turns red when something is wrong.
import type { CityStatus, Stats } from '../protocol';
import {
  clockText,
  demandFill,
  demandLabel,
  dollars,
  fmt,
  perHour,
  populationRows,
  powerAdvice,
  powerFigures,
  saveBadge,
} from './figures';
import type { Popovers } from './popover';

type Lookup = (id: string) => HTMLElement;
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

  /** `openPower` opens the Power category of the build menu, focusing its first item when
   * asked from the keyboard. */
  constructor(
    byId: Lookup,
    popovers: Popovers,
    setSpeed: (speed: number) => void,
    openPower: (focus: boolean) => void,
  ) {
    this.byId = byId;
    this.speeds = SPEEDS.map((s) => byId(`speed-${s}`));
    this.speeds.forEach((b, i) => {
      b.onclick = () => setSpeed(SPEEDS[i]);
    });
    for (const figure of ['population', 'money', 'power'])
      popovers.bind(byId(figure), byId(`${figure}-panel`));
    popovers.bind(byId('menu-button'), byId('menu'), false, byId('save-badge'));
    byId('open-power').onclick = (e?: MouseEvent) => {
      popovers.close();
      openPower(e?.detail === 0);
    };
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
    this.drawDemand(s);
  }

  private drawMoney(s: Stats) {
    const $ = this.byId;
    const net = s.taxes + s.grant - s.upkeep;
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
