// Developer numbers, hidden until the backtick key shows them: frame and tick timings, stuck
// cars, failed routes and cars removed by road edits.
import type { Stats } from '../protocol';
import { debugText } from './figures';

export class DebugPanel {
  /** How long the build menu's pictures took to draw at startup. */
  picturesMs = 0;
  private readonly el: HTMLElement;

  constructor(el: HTMLElement) {
    this.el = el;
    el.hidden = true;
  }

  /** The backtick key toggles the panel; false for any other key. */
  key(key: string): boolean {
    if (key !== '`') return false;
    this.el.hidden = !this.el.hidden;
    return true;
  }

  draw(stats: Stats | undefined, fps: number, tickMs: number) {
    if (this.el.hidden || !stats) return;
    const text = debugText(stats, fps, tickMs, this.picturesMs);
    if (this.el.textContent !== text) this.el.textContent = text;
  }
}
