// A small element builder for the interface code that makes its own controls.

type Props = Record<string, string | boolean | undefined>;

/**
 * Makes a `tag` element. Props become attributes (`class`, `aria-*`, `title`, …); `true` sets an
 * empty attribute, `false` and `undefined` skip it. Strings among `children` become text.
 */
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Props = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === false) continue;
    if (key === 'hidden') el.hidden = true;
    else el.setAttribute(key, value === true ? '' : value);
  }
  el.append(...children);
  return el;
}

/** Sets `aria-pressed` only when it changes. */
export function press(el: HTMLElement, on: boolean) {
  const value = String(on);
  if (el.getAttribute('aria-pressed') !== value) el.setAttribute('aria-pressed', value);
}

/** A control's tooltip: its name, one line about it and its key. */
export function tooltip(label: string, tip: string, key?: string) {
  return `${label}\n${tip}${key ? `\nKey: ${key}` : ''}`;
}
