// Undo and redo (ADR 0051): two buttons at the end of the build bar, Ctrl+Z (Cmd+Z) and
// Ctrl+Shift+Z (Cmd+Shift+Z) in any tool. The sim keeps the history and says what each step
// would do; hovering a button draws what it would take away and put back.
import type { FromWorker, HistoryStatus, StepKind, ToWorker } from '../protocol';
import { h, tooltip } from './dom';
import { dollars } from './figures';
import { iconSvg } from './icons';

/** What each step did, for "Undo: build road". */
const ACTIONS: Record<StepKind, string> = {
  road: 'build road',
  curve: 'build curve',
  grid: 'build grid',
  class: 'change road type',
  bulldoze: 'bulldoze',
  coal: 'place coal plant',
  solar: 'place solar farm',
  wind: 'place wind turbine',
  line: 'build power line',
  pump: 'place pump',
  tower: 'place water tower',
  station: 'place fire station',
};

type HistoryReply = Extract<FromWorker, { type: 'historyResult' | 'historyPreview' }>;

export interface HistoryDrawing {
  setHistoryPreview(remove: Float32Array, restore: Float32Array, ok: boolean): void;
}

export class HistoryButtons {
  private readonly undoButton: HTMLButtonElement;
  private readonly redoButton: HTMLButtonElement;
  private status: HistoryStatus | null = null;
  /** The button last pressed, for marking a refusal. */
  private pressed: HTMLButtonElement | null = null;
  /** The button whose preview is shown, so a new status can ask again. */
  private hovered: boolean | null = null;
  private revision = 0;
  private nextId = 0;
  private readonly send: (msg: ToWorker) => void;
  private readonly draw: HistoryDrawing;
  private readonly done: () => void;

  /** `done` runs after an undo or redo changed the city. */
  constructor(
    bar: HTMLElement,
    send: (msg: ToWorker) => void,
    draw: HistoryDrawing,
    done: () => void,
  ) {
    this.send = send;
    this.draw = draw;
    this.done = done;
    this.undoButton = this.button(false);
    this.redoButton = this.button(true);
    bar.append(
      h('span', { class: 'divider', 'aria-hidden': 'true' }),
      this.undoButton,
      this.redoButton,
    );
    this.set(null);
  }

  /** The frame's history; null for a city not started yet. */
  set(status: HistoryStatus | null) {
    const was = this.status;
    this.status = status;
    this.label(false, status?.undo ?? null, status?.undoRefund ?? null, true);
    this.label(true, status?.redo ?? null, status?.redoCost ?? null, !!status?.redoAffordable);
    const same = (a: HistoryStatus | null, b: HistoryStatus | null) =>
      JSON.stringify(a) === JSON.stringify(b);
    if (this.hovered !== null && !same(was, status)) this.preview(this.hovered);
  }

  receive(msg: HistoryReply) {
    if (msg.type === 'historyResult') {
      const button = this.nextId === msg.id ? this.pressed : null;
      if (msg.ok) this.done();
      else button?.classList.add('refused');
      return;
    }
    if (msg.revision !== this.revision || msg.redo !== this.hovered) return;
    this.draw.setHistoryPreview(msg.remove, msg.restore, msg.ok);
  }

  /** Ctrl+Z (Cmd+Z) undoes, with Shift it redoes; true if the key was one of those. */
  key(e: KeyboardEvent): boolean {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.key.toLowerCase() !== 'z') return false;
    e.preventDefault?.();
    this.press(e.shiftKey);
    return true;
  }

  private press(redo: boolean) {
    const kind = redo ? this.status?.redo : this.status?.undo;
    if (!kind) return;
    const button = redo ? this.redoButton : this.undoButton;
    button.classList.remove('refused');
    this.pressed = button;
    this.send({ type: redo ? 'redo' : 'undo', id: ++this.nextId });
  }

  private preview(redo: boolean | null) {
    this.hovered = redo;
    this.revision++;
    if (redo === null) this.draw.setHistoryPreview(new Float32Array(), new Float32Array(), true);
    else this.send({ type: 'previewHistory', redo, revision: this.revision });
  }

  private button(redo: boolean): HTMLButtonElement {
    const name = redo ? 'Redo' : 'Undo';
    const b = h('button', {
      type: 'button',
      id: name.toLowerCase(),
      class: 'bar-button history-button',
      'aria-keyshortcuts': redo ? 'Control+Shift+Z Meta+Shift+Z' : 'Control+Z Meta+Z',
    });
    b.innerHTML = iconSvg(redo ? 'redo' : 'undo');
    b.onclick = () => this.press(redo);
    const show = () => this.preview(redo);
    const hide = () => {
      if (this.hovered === redo) this.preview(null);
    };
    b.addEventListener('pointerenter', show);
    b.addEventListener('focus', show);
    b.addEventListener('pointerleave', hide);
    b.addEventListener('blur', hide);
    return b;
  }

  /** Names the step and its money; an unaffordable redo is marked short of money. */
  private label(redo: boolean, kind: StepKind | null, money: number | null, affordable: boolean) {
    const b = redo ? this.redoButton : this.undoButton;
    const name = redo ? 'Redo' : 'Undo';
    const key = redo ? 'Ctrl+Shift+Z' : 'Ctrl+Z';
    let label = `Nothing to ${name.toLowerCase()}`;
    let tip = redo ? 'Edits you undo can be made again.' : 'Road and building edits you make.';
    if (kind) {
      label = `${name}: ${ACTIONS[kind]}`;
      tip = !money
        ? 'Costs nothing.'
        : redo
          ? `Costs ${dollars(money)}${affordable ? '.' : ', more than the city has.'}`
          : `Gives back ${dollars(money)}.`;
    }
    b.disabled = kind === null;
    b.classList.toggle('short', redo && kind !== null && !affordable);
    if (b.getAttribute('aria-label') !== label) b.setAttribute('aria-label', label);
    b.title = tooltip(label, tip, key);
  }
}
