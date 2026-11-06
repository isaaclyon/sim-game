// A small stand-in for the browser DOM, so main.ts and the interface code can build and use
// their elements in Node. `index.html` is parsed into the same tree, so tests find the page's
// elements by ID as the game does. Only what the frontend uses is here.
import { readFileSync } from 'node:fs';
import { inspect } from 'node:util';

const VOID = new Set(['br', 'hr', 'img', 'input', 'link', 'meta']);
const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: '\u00a0' };
const decode = (s) => s.replace(/&(\w+);/g, (m, name) => ENTITIES[name] ?? m);

class Text {
  constructor(data) {
    this.data = data;
    this.parentNode = null;
  }
  get textContent() {
    return this.data;
  }
}

/** One compound selector, such as `button:not([disabled])` or `a[href]`. */
function compound(source) {
  const parts = [];
  const re = /^(?:([a-z][\w-]*)|#([\w-]+)|\.([\w-]+)|\[([\w-]+)(?:="([^"]*)")?\]|:not\(([^)]*)\))/i;
  let rest = source.trim();
  while (rest) {
    const m = re.exec(rest);
    if (!m) throw new Error(`fake DOM cannot match selector "${source}"`);
    rest = rest.slice(m[0].length);
    if (m[1]) parts.push((el) => el.localName === m[1].toLowerCase());
    else if (m[2]) parts.push((el) => el.id === m[2]);
    else if (m[3]) parts.push((el) => el.classList.contains(m[3]));
    else if (m[4] && m[5] === undefined) parts.push((el) => el.hasAttribute(m[4]));
    else if (m[4]) parts.push((el) => el.getAttribute(m[4]) === m[5]);
    else {
      const not = compound(m[6]);
      parts.push((el) => !not(el));
    }
  }
  return (el) => parts.every((p) => p(el));
}

/** A selector list such as `input, textarea`; descendant combinators are not supported. */
function matcher(selector) {
  const alternatives = selector.split(',').map(compound);
  return (el) => alternatives.some((m) => m(el));
}

export class FakeElement {
  constructor(document, tag) {
    this.ownerDocument = document;
    this.localName = tag.toLowerCase();
    this.tagName = tag.toUpperCase();
    this.childNodes = [];
    this.parentNode = null;
    this.attributes = {};
    this.style = {};
    this.dataset = {};
    this.events = {};
    this.hidden = false;
    this.disabled = false;
    this.value = '';
    this.onclick = null;
    const el = this;
    this.classList = {
      contains: (name) => el.classNames().includes(name),
      add: (name) => el.classList.toggle(name, true),
      remove: (name) => el.classList.toggle(name, false),
      toggle(name, force) {
        const names = el.classNames().filter((n) => n !== name);
        const on = force ?? !el.classNames().includes(name);
        el.attributes.class = (on ? [...names, name] : names).join(' ');
        return on;
      },
    };
  }

  classNames() {
    return (this.attributes.class ?? '').split(/\s+/).filter(Boolean);
  }

  get id() {
    return this.attributes.id ?? '';
  }
  set id(value) {
    this.attributes.id = value;
  }
  get title() {
    return this.attributes.title ?? '';
  }
  set title(value) {
    this.attributes.title = value;
  }
  get children() {
    return this.childNodes.filter((n) => n instanceof FakeElement);
  }

  setAttribute(key, value) {
    const v = String(value);
    this.attributes[key] = v;
    if (key === 'hidden') this.hidden = true;
    else if (key === 'disabled') this.disabled = true;
    else if (key === 'value') this.value = v;
    else if (key.startsWith('data-'))
      this.dataset[key.slice(5).replace(/-(\w)/g, (_, c) => c.toUpperCase())] = v;
  }
  getAttribute(key) {
    if (key === 'hidden') return this.hidden ? '' : null;
    if (key === 'disabled') return this.disabled ? '' : null;
    return this.attributes[key] ?? null;
  }
  hasAttribute(key) {
    return this.getAttribute(key) !== null;
  }
  removeAttribute(key) {
    if (key === 'hidden') this.hidden = false;
    else if (key === 'disabled') this.disabled = false;
    delete this.attributes[key];
  }

  get textContent() {
    return this.childNodes.map((n) => n.textContent).join('');
  }
  set textContent(value) {
    this.replaceChildren(...(value === '' || value == null ? [] : [String(value)]));
  }
  set innerHTML(markup) {
    this.replaceChildren();
    parseInto(this, markup);
  }

  append(...nodes) {
    for (const node of nodes) {
      const child = typeof node === 'string' ? new Text(node) : node;
      child.parentNode?.removeChild(child);
      child.parentNode = this;
      this.childNodes.push(child);
    }
  }
  replaceChildren(...nodes) {
    for (const child of this.childNodes) child.parentNode = null;
    this.childNodes = [];
    this.append(...nodes);
  }
  removeChild(child) {
    this.childNodes = this.childNodes.filter((n) => n !== child);
    child.parentNode = null;
    return child;
  }
  remove() {
    this.parentNode?.removeChild(this);
  }
  contains(node) {
    for (let n = node; n; n = n.parentNode) if (n === this) return true;
    return false;
  }

  matches(selector) {
    return matcher(selector)(this);
  }
  closest(selector) {
    const match = matcher(selector);
    for (let n = this; n instanceof FakeElement; n = n.parentNode) if (match(n)) return n;
    return null;
  }
  *descendants() {
    for (const child of this.children) {
      yield child;
      yield* child.descendants();
    }
  }
  querySelectorAll(selector) {
    const match = matcher(selector);
    return [...this.descendants()].filter(match);
  }
  querySelector(selector) {
    return this.querySelectorAll(selector)[0] ?? null;
  }

  addEventListener(key, fn) {
    this.events[key] = fn;
  }
  focus() {
    this.ownerDocument.activeElement = this;
  }
  blur() {
    if (this.ownerDocument.activeElement === this) this.ownerDocument.activeElement = null;
  }
  /** A click as the browser makes it: nothing for a disabled control. */
  click(event = {}) {
    if (this.disabled) return;
    const e = { detail: 1, target: this, ...event };
    this.onclick?.(e);
    this.events.click?.(e);
  }
  getBoundingClientRect() {
    return { left: 0, top: 0, right: 0, bottom: 0, width: 0, height: 0 };
  }
  get offsetWidth() {
    return 0;
  }
  setPointerCapture() {}
  select() {}
  /** Failed assertions print `<button#id>` instead of walking the whole page. */
  [inspect.custom]() {
    return `<${this.localName}${this.id ? `#${this.id}` : ''}>`;
  }
}

const TAG = /<!--[\s\S]*?-->|<(\/?)([a-zA-Z][\w-]*)((?:\s+[^\s=>/]+(?:="[^"]*")?)*)\s*(\/?)>/g;
const ATTRIBUTE = /([^\s=>/]+)(?:="([^"]*)")?/g;

/** Parses well-formed markup, such as index.html's body, into `parent`. */
function parseInto(parent, markup) {
  const document = parent.ownerDocument;
  const stack = [parent];
  let last = 0;
  for (const m of markup.matchAll(TAG)) {
    const text = markup.slice(last, m.index);
    last = m.index + m[0].length;
    if (text.trim()) stack.at(-1).append(decode(text));
    if (m[0].startsWith('<!--')) continue;
    const [, closing, tag, attributes, selfClosing] = m;
    if (closing) {
      if (stack.at(-1).localName !== tag.toLowerCase())
        throw new Error(`fake DOM: </${tag}> does not close <${stack.at(-1).localName}>`);
      stack.pop();
      continue;
    }
    const el = new FakeElement(document, tag);
    for (const [, key, value] of attributes.matchAll(ATTRIBUTE))
      el.setAttribute(key, decode(value ?? ''));
    stack.at(-1).append(el);
    if (!selfClosing && !VOID.has(el.localName)) stack.push(el);
  }
  const rest = markup.slice(last);
  if (rest.trim()) stack.at(-1).append(decode(rest));
  if (stack.length !== 1) throw new Error(`fake DOM: <${stack.at(-1).localName}> is not closed`);
}

/** A document whose body is `htmlFile`'s body, without its scripts. */
export function fakeDocument(htmlFile) {
  const html = readFileSync(htmlFile, 'utf8');
  const body = /<body[^>]*>([\s\S]*)<\/body>/.exec(html)?.[1] ?? '';
  const document = {
    activeElement: null,
    visibilityState: 'visible',
    events: {},
    createElement: (tag) => new FakeElement(document, tag),
    getElementById: (id) => (document.body.id === id ? document.body : null) ?? find(id),
    querySelector: (selector) => document.body.querySelector(selector),
    querySelectorAll: (selector) => document.body.querySelectorAll(selector),
    addEventListener(key, fn) {
      this.events[key] = fn;
    },
  };
  const find = (id) => [...document.body.descendants()].find((el) => el.id === id) ?? null;
  document.body = new FakeElement(document, 'body');
  parseInto(document.body, body.replace(/<script[\s\S]*?<\/script>/g, ''));
  document.activeElement = document.body;
  return document;
}
