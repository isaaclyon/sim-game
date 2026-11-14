// Keeps the top bar inside a narrow window. The bar's content changes width with the city
// (money, fire text), so it is measured, not matched against fixed window widths: the bar
// takes the first level whose content fits. Each level is a `fit-N` class in top-bar.css that
// gives up a little more; the last one wraps the bar onto more rows, which always fits.

/** How many levels top-bar.css defines after the full bar (level 0). */
export const FIT_LEVELS = 4;

/** The first level from 0 that does not overflow, or `max` if every earlier one does.
 * `overflows` applies the level and measures it, so the bar is left at the answer. */
export function fitLevel(max: number, overflows: (level: number) => boolean): number {
  for (let level = 0; level < max; level++) if (!overflows(level)) return level;
  overflows(max);
  return max;
}

/** Refits `bar` whenever it or one of its items changes size. */
export function watchFit(bar: HTMLElement) {
  // Tests run without a layout engine; there is nothing to measure.
  if (typeof ResizeObserver === 'undefined') return;
  const apply = (level: number) => {
    for (let i = 1; i <= FIT_LEVELS; i++) bar.classList.toggle(`fit-${i}`, level >= i);
    return bar.scrollWidth > bar.clientWidth;
  };
  const fit = () => {
    fitLevel(FIT_LEVELS, apply);
    // The debug numbers and the inspector card sit below the bar, however tall it is.
    const below = `${Math.round(bar.getBoundingClientRect().bottom) + 10}px`;
    const root = document.documentElement.style;
    if (root.getPropertyValue('--below-top-bar') !== below)
      root.setProperty('--below-top-bar', below);
  };
  // Refitting changes item sizes, which calls this again; the second pass lands on the same
  // level and changes nothing, so it stops there.
  const observer = new ResizeObserver(fit);
  observer.observe(bar);
  for (const item of bar.children) observer.observe(item);
}
