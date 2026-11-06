// Small line icons for the inspector card, drawn in the text colour (24 × 24 view box).
// Constant markup only; text never goes through here.

const PATHS = {
  home: '<path d="M4 11.5 12 5l8 6.5M6.5 10v9h11v-9M10 19v-5h4v5"/>',
  work: '<rect x="3.5" y="7.5" width="17" height="11.5" rx="2"/><path d="M9 7.5V5.5h6v2M3.5 12.5h17"/>',
  shop: '<path d="M5.5 8h13l-1 11.5h-11zM9 8V6.5a3 3 0 0 1 6 0V8"/>',
  walk: '<circle cx="13" cy="4.5" r="1.8"/><path d="m9 21 2.5-6.5 3 3V21M8 11l3-3.5 3 1 2 3 2.5 1M11.5 14.5 12 8"/>',
  car: '<path d="M4 16.5V12l2-5h12l2 5v4.5zM4 12h16"/><circle cx="7.5" cy="16.5" r="1.8"/><circle cx="16.5" cy="16.5" r="1.8"/>',
  clock: '<circle cx="12" cy="12" r="8"/><path d="M12 7.5V12l3 2"/>',
  leave: '<path d="M14 4.5H6v15h8M10 12h10M17 9l3 3-3 3"/>',
  building: '<path d="M5 20V5.5h9V20M14 9.5h5V20M3.5 20h17M8 9h3M8 12.5h3M8 16h3"/>',
  layers: '<path d="m12 4.5 8 4-8 4-8-4zM4 12.5l8 4 8-4M4 16.5l8 4 8-4"/>',
  people:
    '<circle cx="9" cy="8" r="3"/><path d="M3.5 19.5c0-3.2 2.5-5.5 5.5-5.5s5.5 2.3 5.5 5.5"/><circle cx="17" cy="9" r="2.3"/><path d="M16 13.8c2.4.3 4.4 2.3 4.4 5"/>',
  open: '<circle cx="12" cy="12" r="8"/><path d="M8.5 12h7"/>',
  warning: '<path d="M12 4 21 19.5H3zM12 10v4.5M12 17v.5"/>',
} as const;

export type IconName = keyof typeof PATHS;

/** The icon's SVG markup, hidden from screen readers: the line's text says it all. */
export function iconSvg(name: IconName): string {
  return `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">${PATHS[name]}</svg>`;
}
