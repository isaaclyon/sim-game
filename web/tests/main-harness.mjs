import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
import { createContext, SourceTextModule, SyntheticModule } from 'node:vm';

import { RoadClass, Zone } from '../src/protocol.ts';

// Exercise main.ts's event handlers without a WebGL context or a running clock.
export async function tools() {
  function element() {
    return {
      textContent: '',
      hidden: false,
      disabled: false,
      style: {},
      events: {},
      attributes: {},
      classList: { toggle() {} },
      setPointerCapture() {},
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
  // Inspector: the car under the next click, and what the renderer was asked to do.
  const cars = { at: null, visible: true, drawnSelected: [], follows: [], following: [] };
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
    constructor() {
      renderer = this;
    }
    carAt(x, y) {
      cars.clicked = [x, y];
      return cars.at;
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
      const selected = args[3];
      cars.drawnSelected.push(selected);
    }
    drawSignals() {}
    renderer = { domElement: canvas };
    pick(x, z) {
      return { x, z };
    }
    setPreview(stops) {
      previews.push({ type: 'road', stops });
    }
    setLeftDragPans(on) {
      leftDragPans.push(on);
    }
    setZoneDrag(a, b, zone) {
      zoneDrags.push({ a, b, zone });
    }
    setZones(cells, layout) {
      previews.push({ type: 'zones', cells, layout });
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
    },
    URL,
    Worker,
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
  const demandMeter = new SourceTextModule(
    stripTypeScriptTypes(readFileSync(new URL('../src/demand-meter.ts', import.meta.url), 'utf8')),
    { context },
  );
  await module.link((specifier) =>
    specifier === './inspector'
      ? inspector
      : specifier === './demand-meter'
        ? demandMeter
        : specifier === './protocol'
          ? new SyntheticModule(
              ['RoadClass', 'Zone'],
              function () {
                this.setExport('RoadClass', RoadClass);
                this.setExport('Zone', Zone);
              },
              { context },
            )
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
