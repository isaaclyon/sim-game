// The traffic and pollution view buttons: what they show and their labels. Display only.

/** `plain` is the everyday look; the rest are the overlays the pollution button cycles. */
export const POLLUTION_VIEWS = ['plain', 'air', 'ground', 'noise', 'land'] as const;
export type PollutionView = (typeof POLLUTION_VIEWS)[number];

const POLLUTION_VIEW_LABELS: Record<PollutionView, string> = {
  plain: 'pollution view (P)',
  air: 'air pollution (P)',
  ground: 'ground pollution (P)',
  noise: 'noise (P)',
  land: 'land value (P)',
};

export function nextPollutionView(view: PollutionView): PollutionView {
  return POLLUTION_VIEWS[(POLLUTION_VIEWS.indexOf(view) + 1) % POLLUTION_VIEWS.length];
}

interface Views {
  toggleTrafficView(): boolean;
  setPollutionView(view: PollutionView): void;
}

/** Wires both buttons; returns their actions for the keyboard shortcuts (T, P). */
export function bindViewButtons(byId: (id: string) => HTMLElement, renderer: Views) {
  const trafficButton = byId('traffic');
  const pollutionButton = byId('pollution');
  let view: PollutionView = 'plain';
  const traffic = () =>
    trafficButton.setAttribute('aria-pressed', String(renderer.toggleTrafficView()));
  const pollution = () => {
    view = nextPollutionView(view);
    renderer.setPollutionView(view);
    pollutionButton.textContent = POLLUTION_VIEW_LABELS[view];
    pollutionButton.setAttribute('aria-pressed', String(view !== 'plain'));
  };
  trafficButton.onclick = traffic;
  pollutionButton.onclick = pollution;
  return { traffic, pollution };
}
