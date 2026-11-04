import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';

import { PlantKind, RoadClass, Zone } from '../src/protocol.ts';

// Exercise main.ts's event handlers without a WebGL context or a running clock.
export async function tools({ hash = '', clipboard = true } = {}) {
  function element() {
    return {
      textContent: '',
      hidden: false,
      disabled: false,
      style: {},
      dataset: {},
      events: {},
      attributes: {},
      classList: { toggle() {} },
      setPointerCapture() {},
      select() {},
      setAttribute(key, value) {
        this.attributes[key] = value;
      },
      addEventListener(key, fn) {
        this.events[key] = fn;
      },
    };
  }
  const elements = Object.fromEntries(
    [
      'hud',
      'demand',
      'demand-residential',
      'demand-commercial',
      'demand-industrial',
      'road',
      'curve',
      'main-road',
      'bulldoze',
      'road-help',
      'inspector',
      'inspector-text',
      'follow',
      'inspector-close',
      'sound',
      'traffic',
      'save-city',
      'new-city',
      'save-status',
      'share-status',
      'share-url',
      'download-city',
      'copy-link',
      'open-city',
      'open-city-file',
      'command-log',
      'plant-coal',
      'plant-solar',
      'plant-wind',
      'power-line',
      'power-view',
    ].map((id) => [id, element()]),
  );
  const zoneButtons = Object.values(Zone).map((zone) => ({
    ...element(),
    dataset: { zone: String(zone) },
  }));
  const canvas = element();
  const leftDragPans = [];
  const zoneDrags = [];
  const messages = [];
  const previews = [];
  const events = {};
  const documentEvents = {};
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
    setPreview(stops, _a, _b, roadClass) {
      previews.push({ type: 'road', stops, roadClass });
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
  }
  const context = createContext({
    document: {
      body: {},
      getElementById: (id) => elements[id],
      querySelectorAll: (selector) => (selector === '[data-zone]' ? zoneButtons : []),
      visibilityState: 'visible',
      addEventListener: (key, fn) => {
        documentEvents[key] = fn;
      },
      createElement: () => ({
        click() {
          page.downloads.push({ name: this.download, href: this.href });
        },
      }),
    },
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
  const module = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/main.ts', import.meta.url), 'utf8')),
    {
      context,
      initializeImportMeta(meta) {
        meta.url = import.meta.url;
        meta.env = { DEV: false };
      },
    },
  );
  const inspector = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/inspector.ts', import.meta.url), 'utf8')),
    { context },
  );
  const cityControls = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/city-controls.ts', import.meta.url), 'utf8')),
    { context },
  );
  const cityShare = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/city-share.ts', import.meta.url), 'utf8')),
    { context },
  );
  const hud = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/hud.ts', import.meta.url), 'utf8')),
    { context },
  );
  const powerTools = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/power-tools.ts', import.meta.url), 'utf8')),
    { context },
  );
  const protocol = new SyntheticModule(
    ['RoadClass', 'Zone', 'PlantKind'],
    function () {
      this.setExport('RoadClass', RoadClass);
      this.setExport('Zone', Zone);
      this.setExport('PlantKind', PlantKind);
    },
    { context },
  );
  const demandMeter = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/demand-meter.ts', import.meta.url), 'utf8')),
    { context },
  );
  await module.link((specifier) =>
    specifier === './city-controls'
      ? cityControls
      : specifier === './city-share'
        ? cityShare
        : specifier === './hud'
          ? hud
          : specifier === './inspector'
            ? inspector
            : specifier === './ambient-sound'
              ? new SyntheticModule(
                  ['AmbientSound'],
                  function () {
                    this.setExport('AmbientSound', AmbientSound);
                  },
                  { context },
                )
              : specifier === './demand-meter'
                ? demandMeter
                : specifier === './protocol'
                  ? protocol
                  : specifier === './power-tools'
                    ? powerTools
                    : new SyntheticModule(
                        ['Renderer'],
                        function () {
                          this.setExport('Renderer', Renderer);
                        },
                        { context },
                      ),
  );
  await module.evaluate();
  const move = (x, y) => canvas.events.pointermove({ clientX: x, clientY: y });
  const click = (x, y, button = 0) => {
    const e = { clientX: x, clientY: y, button };
    canvas.events.pointerdown(e);
    canvas.events.pointerup(e);
  };
  return {
    elements,
    confirm,
    page,
    frame: () => frame(),
    messages,
    previews,
    zoneButtons,
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
      context.document.visibilityState = 'hidden';
      documentEvents.visibilitychange();
    },
  };
}
