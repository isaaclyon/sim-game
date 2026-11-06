// The build menu (M4.6): the category bar, the item row and the options row, built from the
// list in `catalog.ts`. It owns which button is selected; the tools keep their canvas handling
// and sim commands, and report what is in use through `show`.
import {
  type Category,
  type Item,
  type Option,
  type Picture,
  pictureKey,
  type RoadMode,
  type Tool,
} from './catalog';
import { h, press, tooltip } from './dom';
import type { Popovers } from './popover';

/** What is in use, for marking the selected buttons. */
export interface Selection {
  tool: Tool | null;
  /** The road tool's shape while a road tool is in use. */
  roadMode: RoadMode | null;
  /** The `Snap` flags that are on. */
  snap: number;
}

export interface MenuActions {
  /** Starts the item's tool; null puts every tool away. */
  pick(item: Item | null): void;
  option(option: Option): void;
}

/** Whether two tools are the same, such as the zone brush for one zone. */
export function sameTool(a: Tool | null, b: Tool | null): boolean {
  if (!a || !b) return a === b;
  const fields = a as unknown as Record<string, unknown>;
  const other = b as unknown as Record<string, unknown>;
  return Object.keys(fields).every((k) => fields[k] === other[k]);
}

function optionOn(o: Option, s: Selection) {
  return o.action.kind === 'roadMode'
    ? s.roadMode === o.action.mode
    : (s.snap & o.action.flag) !== 0;
}

/** A single letter shortcut, for `aria-keyshortcuts`; "Z twice" has none. */
const shortcut = (key?: string) => (key && key.length === 1 ? key : undefined);

export class BuildMenu {
  /** The one-line hint in the options row; the tools write to it. */
  readonly hint: HTMLElement;
  readonly viewsButton: HTMLElement;
  private open: string | null = null;
  private selection: Selection = { tool: null, roadMode: null, snap: 0 };
  private readonly categories: Category[];
  private readonly actions: MenuActions;
  private readonly items: [Item, HTMLElement][] = [];
  private readonly options: [Option, HTMLElement][] = [];
  private readonly categoryButtons = new Map<string, HTMLElement>();
  private readonly rows = new Map<string, HTMLElement>();
  private readonly optionGroups = new Map<string, HTMLElement>();
  private readonly images: [Picture, HTMLImageElement][] = [];
  /** Panels opened from a category's options, such as Snaps, by category. */
  private readonly optionPanels: [string, HTMLElement][] = [];
  private readonly popovers: Popovers;
  private readonly optionsRow: HTMLElement;
  private readonly bulldozeButton: HTMLElement;

  constructor(
    root: HTMLElement,
    categories: Category[],
    bulldoze: Item,
    actions: MenuActions,
    popovers: Popovers,
  ) {
    // A category with no items is not shown.
    this.categories = categories.filter((c) => c.items.length > 0);
    this.actions = actions;
    this.popovers = popovers;
    // Not a live region: the road hint carries the cost, which changes with every cursor move.
    this.hint = h('p', { id: 'build-hint', class: 'hint' });
    this.optionsRow = h(
      'div',
      { id: 'build-options', class: 'glass options-row', hidden: true },
      ...this.categories.map((c) => this.optionGroup(c, popovers)),
      this.hint,
    );
    const bar = h(
      'nav',
      { class: 'glass category-bar', 'aria-label': 'Build' },
      ...this.categories.map((c) => this.categoryButton(c)),
      h('span', { class: 'divider', 'aria-hidden': 'true' }),
    );
    this.bulldozeButton = this.barButton(bulldoze.id, bulldoze.label, bulldoze.picture, {
      title: tooltip(bulldoze.label, bulldoze.tip, bulldoze.key),
      'aria-pressed': 'false',
      'aria-keyshortcuts': shortcut(bulldoze.key),
    });
    this.bulldozeButton.onclick = () => this.pick(bulldoze);
    this.items.push([bulldoze, this.bulldozeButton]);
    this.viewsButton = this.barButton(
      'views',
      'Views',
      { kind: 'icon', name: 'views' },
      {
        title: tooltip('Views', 'Pollution, land value, traffic and power.', 'P, T, O'),
        'aria-expanded': 'false',
      },
    );
    bar.append(this.bulldozeButton, this.viewsButton);
    root.append(this.optionsRow, ...this.categories.map((c) => this.itemRow(c)), bar);
    this.render();
  }

  /** Marks what is in use. A tool's category opens with it and closes when it is put away. */
  show(selection: Selection) {
    const had = this.selection.tool !== null;
    this.selection = selection;
    if (selection.tool) this.open = this.categoryOf(selection.tool);
    else if (had) this.open = null;
    this.render();
  }

  /** Opens a category's items, putting away a tool from another category. `focus` moves the
   * keyboard on into the items that just appeared. */
  openCategory(id: string, focus = false) {
    const using = this.categoryOf(this.selection.tool);
    if (this.selection.tool && using !== id) this.actions.pick(null);
    this.open = id;
    this.render();
    if (focus) this.rows.get(id)?.querySelector('button')?.focus();
  }

  /** Closes the open category; false if none was open. */
  closeCategory(): boolean {
    if (this.open === null) return false;
    this.open = null;
    this.render();
    return true;
  }

  /** Every picture model the menu shows, once each. */
  get pictures(): Picture[] {
    const unique = new Map(this.images.map(([p]) => [pictureKey(p), p]));
    return [...unique.values()];
  }

  /** Puts the drawn pictures in place; a picture that failed to draw keeps only its label. */
  setPictures(urls: Map<string, string>) {
    for (const [picture, img] of this.images) {
      const url = urls.get(pictureKey(picture));
      if (!url) continue;
      img.src = url;
      img.hidden = false;
    }
  }

  private pick(item: Item) {
    this.actions.pick(sameTool(item.tool, this.selection.tool) ? null : item);
  }

  /** The category holding `tool`; the open one first, in case two hold the same tool. */
  private categoryOf(tool: Tool | null): string | null {
    if (!tool) return null;
    const holds = (c: Category) => c.items.some((i) => sameTool(i.tool, tool));
    const open = this.categories.find((c) => c.id === this.open);
    if (open && holds(open)) return open.id;
    return this.categories.find(holds)?.id ?? null;
  }

  private render() {
    const s = this.selection;
    const using = this.categoryOf(s.tool);
    // A panel opened from options that are going away closes with them.
    for (const [id, panel] of this.optionPanels)
      if (id !== using && this.popovers.isOpen(panel)) this.popovers.close();
    const focused = document.activeElement as HTMLElement | null;
    const holder = [...this.rows, ...this.optionGroups].find(([, el]) => el.contains(focused));
    for (const [id, button] of this.categoryButtons) {
      const open = String(id === this.open);
      if (button.getAttribute('aria-expanded') !== open) button.setAttribute('aria-expanded', open);
    }
    for (const [id, row] of this.rows) row.hidden = id !== this.open;
    for (const [item, button] of this.items) press(button, sameTool(item.tool, s.tool));
    for (const [option, button] of this.options) press(button, optionOn(option, s));
    for (const [id, group] of this.optionGroups) group.hidden = id !== using;
    this.optionsRow.hidden = s.tool === null;
    // Focus in a row that just closed moves to its category, so the keyboard keeps its place.
    if (holder && focused?.closest('[hidden]')) this.categoryButtons.get(holder[0])?.focus();
  }

  private picture(p: Picture, alt: string) {
    const img = h('img', { class: 'picture', alt, draggable: 'false', hidden: true });
    this.images.push([p, img]);
    return img;
  }

  /** A button in the category bar: a small picture over its label. */
  private barButton(
    id: string,
    label: string,
    icon: Picture,
    props: Record<string, string | undefined>,
  ) {
    return h(
      'button',
      { type: 'button', id, class: 'bar-button', 'aria-label': label, ...props },
      this.picture(icon, label),
      h('span', { class: 'label' }, label),
    );
  }

  private categoryButton(c: Category) {
    const b = this.barButton(`category-${c.id}`, c.label, c.icon, {
      title: tooltip(c.label, c.tip),
      'aria-expanded': 'false',
      'aria-controls': `items-${c.id}`,
    });
    // From the keyboard (detail 0), carry on into the items that just appeared.
    b.onclick = (e?: MouseEvent) =>
      this.open === c.id ? this.close() : this.openCategory(c.id, e?.detail === 0);
    this.categoryButtons.set(c.id, b);
    return b;
  }

  /** Closing a category puts its tool away. */
  private close() {
    if (this.selection.tool) this.actions.pick(null);
    this.closeCategory();
  }

  private itemRow(c: Category) {
    const row = h('div', {
      id: `items-${c.id}`,
      class: 'glass item-row',
      role: 'group',
      'aria-label': c.label,
      hidden: true,
    });
    for (const item of c.items) {
      const b = h(
        'button',
        {
          type: 'button',
          id: item.id,
          class: 'item',
          'aria-label': item.label,
          'aria-pressed': 'false',
          'aria-keyshortcuts': shortcut(item.key),
          title: tooltip(item.label, item.tip, item.key),
        },
        h('span', { class: 'frame' }, this.picture(item.picture, item.label)),
        h('span', { class: 'label' }, item.label),
      );
      if (item.stripe) {
        const stripe = h('span', { class: 'stripe', 'aria-hidden': 'true' });
        stripe.style.background = item.stripe;
        b.append(stripe);
      }
      b.onclick = () => this.pick(item);
      this.items.push([item, b]);
      row.append(b);
    }
    this.rows.set(c.id, row);
    return row;
  }

  /** Plain options sit in a row; options that share a group open from one button. */
  private optionGroup(c: Category, popovers: Popovers) {
    const group = h('div', { id: `options-${c.id}`, class: 'options', hidden: true });
    const options = c.options ?? [];
    // A category without options keeps its empty group hidden, so the hint has no divider.
    if (options.length) this.optionGroups.set(c.id, group);
    const button = (o: Option) => {
      const b = h(
        'button',
        {
          type: 'button',
          id: o.id,
          'aria-pressed': 'false',
          'aria-keyshortcuts': shortcut(o.key),
          title: tooltip(o.label, o.tip, o.key),
        },
        o.label,
      );
      b.onclick = () => this.actions.option(o);
      this.options.push([o, b]);
      return b;
    };
    const plain = options.filter((o) => !o.group);
    if (plain.length)
      group.append(
        h(
          'div',
          { class: 'segmented', role: 'group', 'aria-label': `${c.label} options` },
          ...plain.map(button),
        ),
      );
    for (const name of new Set(options.flatMap((o) => (o.group ? [o.group] : [])))) {
      const id = name.toLowerCase().replace(/\W+/g, '-');
      const panel = h(
        'section',
        { id: `${id}-panel`, class: 'glass panel option-panel', 'aria-label': name, hidden: true },
        h('h2', {}, name),
        h('div', { class: 'choices' }, ...options.filter((o) => o.group === name).map(button)),
      );
      document.body.append(panel);
      const open = h(
        'button',
        { type: 'button', id, class: 'drop', 'aria-controls': panel.id, 'aria-expanded': 'false' },
        name,
      );
      popovers.bind(open, panel, true);
      this.optionPanels.push([c.id, panel]);
      group.append(open);
    }
    return group;
  }
}
