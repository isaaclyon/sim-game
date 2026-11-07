import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { extname } from 'node:path';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';

import { PlantKind, RoadClass, Snap, Zone } from '../src/protocol.ts';
import { fakeDocument } from './fake-dom.mjs';

// Exercise main.ts's event handlers without a WebGL context or a running clock. `catalog`, if
// given, may change the build menu's list (`src/ui/catalog.ts`) before the menu is built.
export async function tools({ hash = '', clipboard = true, catalog } = {}) {
  const document = fakeDocument(new URL('../index.html', import.meta.url));
  const createElement = document.createElement;
  // A download is a link the game clicks; record it instead of leaving the page.
  document.createElement = (tag) => {
    const el = createElement(tag);
    if (tag === 'a') el.click = () => page.downloads.push({ name: el.download, href: el.href });
    return el;
  };
  /** The page's elements by ID, as main.ts finds them. */
  const elements = new Proxy({}, { get: (_, id) => document.getElementById(id) });
  const canvas = createElement('canvas');
  const leftDragPans = [];
  const zoneDrags = [];
  const messages = [];
  const previews = [];
  const events = {};
  const pans = [];
  const clock = { now: 0, cameraMoved: false };
  const confirm = { answer: false, prompts: [] };
  // City sharing: the page URL, downloads made and text copied.
  const page = { hash, replaced: [], downloads: [], copied: [] };
  // Inspector: the car under the next click, and what the renderer was asked to do.
  const cars = {
    at: null,
    visible: true,
    drawnSelected: [],
    follows: [],
    following: [],
    draws: [],
  };
  const buildings = { at: null };
  // Ambient sound: what main.ts asked of it.
  const sound = { toggles: 0, buildings: [], updates: [] };
  const lightingTimes = [];
  // The build menu's pictures: which models it asked for, and the data URLs it got back.
  const pictures = { asked: [] };
  class AmbientSound {
    toggle() {
      sound.toggles++;
    }
    setBuildings(data) {
      sound.buildings.push(data);
    }
    update(...args) {
      sound.updates.push(args);
    }
  }
  let renderer;
  let worker;
  let frame;
  class Worker {
    constructor() {
      worker = this;
    }
    postMessage(msg) {
      messages.push(msg);
    }
  }
  class Renderer {
    onUserPan = null;
    setTime(time) {
      lightingTimes.push(time);
    }
    constructor() {
      renderer = this;
    }
    carAt(x, y) {
      cars.clicked = [x, y];
      return cars.at;
    }
    buildingAt() {
      return buildings.at;
    }
    follow(agent, seconds) {
      cars.follows.push([agent, seconds]);
      return cars.visible;
    }
    setFollowing(on) {
      cars.following.push(on);
    }
    render() {
      return clock.cameraMoved;
    }
    keyCamera = {
      update(right, forward, turn, seconds) {
        pans.push([right, forward, turn, seconds]);
        return right !== 0 || forward !== 0 || turn !== 0;
      },
      stop() {
        pans.push('stop');
      },
      stopPan() {
        pans.push('stopPan');
      },
    };
    drawAgents(...args) {
      cars.draws.push(args);
      const selected = args[3];
      cars.drawnSelected.push(selected);
    }
    drawSignals() {}
    drawPictures(list) {
      pictures.asked.push(list);
      return new Map(
        list.map((p) => [Object.values(p).join(':'), `data:image/png;base64,${p.kind}`]),
      );
    }
    setRoads(segments, bounds, fitCamera) {
      previews.push({ type: 'roads', segments, bounds, fitCamera });
    }
    trafficView = false;
    toggleTrafficView() {
      this.trafficView = !this.trafficView;
      return this.trafficView;
    }
    setBuildings(_data, _ids, rise) {
      previews.push({ type: 'buildings', rise });
    }
    listener() {
      return { x: 1, y: 2, distance: 300 };
    }
    renderer = { domElement: canvas };
    pick(x, z) {
      return { x, z };
    }
    setPreview(stops, a, b, roadClass, reclass) {
      previews.push({ type: 'road', stops, a, b, roadClass, reclass });
    }
    setGuides(lines) {
      previews.push({ type: 'guides', lines });
    }
    setAngles(marks) {
      previews.push({ type: 'angles', marks });
    }
    setZoningVisible(visible) {
      previews.push({ type: 'zoningVisible', visible });
    }
    setLeftDragPans(on) {
      leftDragPans.push(on);
    }
    setZoneDrag(a, b, zone) {
      zoneDrags.push({ a, b, zone });
    }
    setZones(cells, layout, flash) {
      previews.push({ type: 'zones', cells, layout, flash });
    }
    setBulldozePreview(segment) {
      previews.push({ type: 'bulldoze', segment });
    }
    setUpgradePreview(segment, ok, roadClass) {
      previews.push({ type: 'upgrade', segment, ok, roadClass });
    }
    setGridPreview(roads, dropped, edge, ok, roadClass) {
      previews.push({ type: 'grid', roads, dropped, edge, ok, roadClass });
    }
    setBlockOutline(outline, zone) {
      previews.push({ type: 'block', outline, zone });
    }
    setLots(rings) {
      previews.push({ type: 'lots', rings });
    }
    setPower(plants, lines, buildings) {
      previews.push({ type: 'power', plants, lines, buildings });
    }
    setPowerView(on) {
      previews.push({ type: 'powerView', on });
    }
    setPlantPreview(outline, ok) {
      previews.push({ type: 'plant', outline, ok });
    }
    setLinePreview(ends, a, b) {
      previews.push({ type: 'line', ends, a, b });
    }
    setPollution(cells, layout) {
      previews.push({ type: 'pollution', cells, layout });
    }
    setPollutionView(view) {
      previews.push({ type: 'pollutionView', view });
    }
  }
  const context = createContext({
    innerWidth: 1000,
    innerHeight: 700,
    document,
    location: {
      get hash() {
        return page.hash;
      },
      href: `https://example.test/game/${hash}`,
      pathname: '/game/',
      search: '',
    },
    history: {
      replaceState(_state, _title, url) {
        page.replaced.push(url);
        page.hash = '';
      },
    },
    navigator: {
      clipboard: {
        async writeText(text) {
          if (!clipboard) throw new Error('denied');
          page.copied.push(text);
        },
      },
    },
    Blob,
    Response,
    CompressionStream,
    DecompressionStream,
    atob,
    btoa,
    setTimeout: () => {},
    URL,
    Worker,
    confirm: (prompt) => {
      confirm.prompts.push(prompt);
      return confirm.answer;
    },
    performance: { now: () => clock.now },
    requestAnimationFrame: (fn) => {
      frame = fn;
    },
    addEventListener: (key, fn) => {
      events[key] = fn;
    },
  });
  const mainUrl = new URL('../src/main.ts', import.meta.url);
  const modules = new Map();
  for (const [name, exports] of [
    ['renderer', { Renderer }],
    ['ambient-sound', { AmbientSound }],
    ['protocol', { PlantKind, RoadClass, Snap, Zone }],
  ]) {
    const identifier = new URL(`./${name}.ts`, mainUrl).href;
    modules.set(
      identifier,
      new SyntheticModule(
        Object.keys(exports),
        function () {
          for (const [name, value] of Object.entries(exports)) this.setExport(name, value);
        },
        { context, identifier },
      ),
    );
  }
  function source(url) {
    if (modules.has(url.href)) return modules.get(url.href);
    const module = new SourceTextModule(stripTypeScriptTypes(readFileSync(url, 'utf8')), {
      context,
      identifier: url.href,
      initializeImportMeta(meta) {
        if (url.href === mainUrl.href) {
          meta.url = import.meta.url;
          meta.env = { DEV: false };
        }
      },
    });
    modules.set(url.href, module);
    return module;
  }
  const module = source(mainUrl);
  await module.link((specifier, referencingModule) => {
    if (!specifier.startsWith('./') && !specifier.startsWith('../')) {
      throw new Error(`Unsupported import "${specifier}" from ${referencingModule.identifier}`);
    }
    const url = new URL(specifier, referencingModule.identifier);
    if (!extname(url.pathname)) url.pathname += '.ts';
    return source(url);
  });
  if (catalog) {
    const list = source(new URL('../src/ui/catalog.ts', import.meta.url));
    await list.evaluate();
    catalog(list.namespace);
  }
  await module.evaluate();
  const move = (x, y) => canvas.events.pointermove({ clientX: x, clientY: y });
  const click = (x, y, button = 0, shiftKey = false) => {
    const e = { clientX: x, clientY: y, button, shiftKey };
    canvas.events.pointerdown(e);
    canvas.events.pointerup(e);
  };
  /** A click on a page control as a person makes it, which first closes an open panel. Only a
   * control on screen can be clicked. */
  const press = (el) => {
    for (let n = el; n !== document.body; n = n.parentNode)
      if (n.hidden) throw new Error(`#${el.id} is not on screen`);
    document.events.pointerdown?.({ target: el });
    el.click();
  };
  return {
    document,
    elements,
    press,
    /** Presses the control with this ID. */
    pressId: (id) => press(elements[id]),
    /** The inspector card's title and lines, one per line of text. */
    card: () =>
      [elements['inspector-title'], ...elements['inspector-lines'].children]
        .map((el) => el.textContent)
        .join('\n'),
    pictures,
    confirm,
    page,
    frame: () => frame(),
    messages,
    previews,
    pans: leftDragPans,
    cameraPans: pans,
    canvas,
    clock,
    zoneDrags,
    cars,
    buildings,
    sound,
    lightingTimes,
    userPan: () => renderer.onUserPan?.(),
    down: (x, y) => canvas.events.pointerdown({ clientX: x, clientY: y, button: 0 }),
    up: (x, y) => canvas.events.pointerup({ clientX: x, clientY: y, button: 0 }),
    move,
    click,
    reply: (data) => worker.onmessage({ data }),
    key: (key, options = {}) => events.keydown({ key, ...options }),
    keyup: (key, options = {}) => events.keyup({ key, ...options }),
    event: (key, e = {}) => events[key](e),
    hide() {
      document.visibilityState = 'hidden';
      document.events.visibilitychange();
    },
  };
}
