// Panels opened by a button: figure details, the ☰ menu, Views and Snaps. One is open at a
// time; a second click on its button, Esc or a click elsewhere closes it.

const GAP_PX = 8;
const EDGE_PX = 10;

interface Open {
  /** The button that opened it: the panel sits beside it, and focus goes back to it. */
  button: HTMLElement;
  panel: HTMLElement;
  above: boolean;
}

export class Popovers {
  private current: Open | null = null;
  /** Every button that opens each panel. */
  private readonly buttons = new Map<HTMLElement, HTMLElement[]>();

  constructor() {
    // Capture, so a click on the city closes a panel before the tools see it.
    document.addEventListener(
      'pointerdown',
      (e) => {
        const target = e.target as Node | null;
        const open = this.current;
        if (!open || !target) return;
        if (open.panel.contains(target) || this.openers(open).some((b) => b.contains(target)))
          return;
        this.close();
      },
      true,
    );
  }

  /** Makes `button`, and any `others`, open and close `panel`, below the button or, for the
   * build menu, above it. */
  bind(button: HTMLElement, panel: HTMLElement, above = false, ...others: HTMLElement[]) {
    panel.hidden = true;
    const buttons = [button, ...others];
    this.buttons.set(panel, buttons);
    for (const b of buttons) {
      b.setAttribute('aria-expanded', 'false');
      // A click from the keyboard (detail 0) moves focus into the panel, so it can be reached.
      b.onclick = (e?: MouseEvent) => this.toggle(b, panel, above, e?.detail === 0);
    }
  }

  isOpen(panel: HTMLElement) {
    return this.current?.panel === panel;
  }

  toggle(button: HTMLElement, panel: HTMLElement, above = false, focus = false) {
    if (this.isOpen(panel)) this.close();
    else this.show(button, panel, above, focus);
  }

  show(button: HTMLElement, panel: HTMLElement, above = false, focus = false) {
    this.close();
    this.current = { button, panel, above };
    panel.hidden = false;
    for (const b of this.openers(this.current)) b.setAttribute('aria-expanded', 'true');
    this.place();
    if (!focus) return;
    const control = panel.querySelector<HTMLElement>('button:not([disabled]):not([hidden])');
    if (control) return control.focus();
    // A panel of figures only: focus it whole, so a screen reader reads it.
    panel.tabIndex = -1;
    panel.focus();
  }

  /** Closes the open panel; false if none was open. */
  close(): boolean {
    const open = this.current;
    if (!open) return false;
    this.current = null;
    // Focus inside the panel goes back to the button that opened it.
    const focused = open.panel.contains(document.activeElement);
    open.panel.hidden = true;
    for (const b of this.openers(open)) b.setAttribute('aria-expanded', 'false');
    if (focused) open.button.focus();
    return true;
  }

  private openers(open: Open): HTMLElement[] {
    return this.buttons.get(open.panel) ?? [open.button];
  }

  /** Keeps the open panel beside its button and on screen, as its content changes size. */
  place() {
    const open = this.current;
    if (!open) return;
    const r = open.button.getBoundingClientRect();
    const width = open.panel.offsetWidth;
    const left = Math.min(
      Math.max(r.left + r.width / 2 - width / 2, EDGE_PX),
      innerWidth - width - EDGE_PX,
    );
    open.panel.style.left = `${Math.max(left, EDGE_PX)}px`;
    if (open.above) {
      open.panel.style.top = '';
      open.panel.style.bottom = `${innerHeight - r.top + GAP_PX}px`;
    } else {
      open.panel.style.bottom = '';
      open.panel.style.top = `${r.bottom + GAP_PX}px`;
    }
  }
}
