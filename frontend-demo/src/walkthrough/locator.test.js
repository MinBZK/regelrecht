import { afterEach, describe, expect, it } from 'vitest';
import { describe as describeEl, loosen, resolve, signature } from './locator.js';
import { installClock, uninstallClock } from './clock.js';
import { centerOf, graphView, showView } from './graphBridge.js';

afterEach(() => {
  document.body.innerHTML = '';
});

/** A design-system-like host: a custom element with a button in its shadow root. */
function host(tag, label) {
  const el = document.createElement(tag);
  const root = el.attachShadow({ mode: 'open' });
  const button = document.createElement('button');
  button.setAttribute('aria-label', label);
  root.appendChild(button);
  document.body.appendChild(el);
  return { el, button };
}

describe('locator', () => {
  it('names an element by what a person reads, not by its place', () => {
    const a = document.createElement('a');
    a.href = `${window.location.origin}/wetten/zorgtoeslagwet`;
    a.textContent = '  Zorgtoeslag\n  ';
    document.body.appendChild(a);
    // A link without its origin, so a recording from localhost replays on
    // the demo's own domain.
    expect(signature(a).href).toBe('/wetten/zorgtoeslagwet');
  });

  it('scrolls to the line that was in view, not to the same pixels', async () => {
    const { scrollAnchor, findAnchor, scrollTarget } = await import('./locator.js');
    const box = document.createElement('div');
    for (const text of ['artikel 2', 'parameters:', '-bsn', 'parameters:', 'actions:']) {
      const line = document.createElement('span');
      line.textContent = text;
      box.appendChild(line);
    }
    document.body.appendChild(box);
    const lines = [...box.children];
    // jsdom has no layout: a box 100px high, lines of 20px, the second
    // "parameters:" across its middle.
    box.getBoundingClientRect = () => ({ top: 0, left: 0, width: 400, height: 100, bottom: 100 });
    lines.forEach((l, i) => (l.getBoundingClientRect = () => ({ top: i * 15 - 5, bottom: i * 15 + 15, height: 20 })));
    lines[3].getBoundingClientRect = () => ({ top: 40, bottom: 60, height: 20 });
    const anchor = scrollAnchor(box);
    expect(anchor).toEqual({ tag: 'span', text: 'parameters:', index: 1, at: 0.4 });
    expect(findAnchor(box, anchor)).toBe(lines[3]);
    // In another layout that line sits 300px down: scroll it back to 40%.
    lines[3].getBoundingClientRect = () => ({ top: 300 });
    expect(scrollTarget(box, { top: 999, anchor })).toBe(260);
    // An older take without an anchor: the position scaled to this layout.
    Object.defineProperty(box, 'scrollHeight', { value: 1100 });
    Object.defineProperty(box, 'clientHeight', { value: 100 });
    expect(scrollTarget(box, { top: 250, max: 500 })).toBe(500);
  });

  it('finds a page again when its text changed, and a field by a renumbered name', () => {
    const page = document.createElement('rr-test-page');
    page.textContent = 'Accijnsplicht en tarief alcoholhoudende dranken Douane Bronnen Tarief bier Tarief wijn';
    document.body.appendChild(page);
    const recorded = [{ tag: 'rr-test-page', textContent: 'Accijnsplicht en tarief alcoholhoudende drankenDouaneBronnenTarief bierTarief wi', nth: 0 }];
    // A law was added: the page's text changed. Exact finds nothing, loose finds the page.
    page.textContent = 'Precariobelasting ' + page.textContent;
    expect(resolve(recorded)).toBeNull();
    expect(resolve(recorded, { loose: true })).toBe(page);
    const radio = document.createElement('input');
    radio.type = 'radio';
    radio.name = 'nldd-segmented-3';
    document.body.appendChild(radio);
    expect(signature(radio).name).toBeUndefined();
    expect(resolve([{ tag: 'input', name: 'nldd-segmented-2', type: 'radio', nth: 0 }])).toBe(radio);
  });

  it('clicks the component when the part inside it that was clicked is gone', () => {
    const { el } = host('rr-test-item', 'Alles');
    const recorded = [{ tag: 'rr-test-item', nth: 0 }, { tag: 'input', type: 'radio', nth: 0 }];
    expect(resolve(recorded)).toBeNull();
    expect(resolve(recorded, { loose: true })).toBe(el);
  });

  it('leaves out an id a component makes up on every render', () => {
    const input = document.createElement('input');
    input.id = 'nldd-field-input-3ec46f7f-58fa-4d60-9122-9bcc611bab37';
    input.setAttribute('aria-label', 'Standaardpremie 2025');
    document.body.appendChild(input);
    expect(signature(input).id).toBeUndefined();
    // A take recorded with the old id still finds the field under its new one.
    const recorded = [{ tag: 'input', id: 'nldd-field-input-00000000-1111-2222-3333-444444444444', '@aria-label': 'Standaardpremie 2025', nth: 0 }];
    expect(resolve(recorded)).toBe(input);
  });

  it('finds a button inside a shadow root again, through its host', () => {
    host('rr-test-bar', 'Wetten, 79');
    const { button } = host('rr-test-bar', 'Graaf');
    const steps = describeEl(button);
    expect(steps.map((s) => s.tag)).toEqual(['rr-test-bar', 'button']);
    expect(resolve(steps)).toBe(button);
  });

  it('falls back to a looser match when a count in a label changed', () => {
    const { button } = host('rr-test-bar', 'Wetten, 79');
    const steps = describeEl(button);
    button.setAttribute('aria-label', 'Wetten, 81');
    // Only when asked: an exact miss first means "wait, it is still loading".
    expect(resolve(steps)).toBeNull();
    expect(resolve(steps, { loose: true })).toBe(button);
  });

  it('never loosens a description down to a bare tag', () => {
    expect(loosen({ tag: 'button', textContent: 'Aanvragen' })).toBeNull();
    expect(loosen({ tag: 'button', '@role': 'tab' })).toBeNull();
    expect(loosen({ tag: 'button', '@aria-label': 'Wetten, 81' })).toEqual({ tag: 'button', __masked: true, '@aria-label': 'Wetten, #' });
  });

  it('tells two alike elements apart by their order', () => {
    const first = document.createElement('button');
    const second = document.createElement('button');
    first.textContent = 'Aanvragen';
    second.textContent = 'Aanvragen';
    document.body.append(first, second);
    expect(resolve(describeEl(second))).toBe(second);
  });

  it('returns null when the element is gone', () => {
    const { button } = host('rr-test-bar', 'Simulatie');
    const steps = describeEl(button);
    document.body.innerHTML = '';
    expect(resolve(steps)).toBeNull();
  });
});

describe('replay clock', () => {
  afterEach(() => uninstallClock());

  it('answers with the recorded moment, and lets go again', () => {
    const recorded = Date.UTC(2026, 9, 2, 10, 0, 0);
    installClock(() => recorded + 5000);
    expect(Date.now()).toBe(recorded + 5000);
    expect(new Date().toISOString()).toBe('2026-10-02T10:00:05.000Z');
    // A date with an argument is left alone.
    expect(new Date(0).getTime()).toBe(0);
    uninstallClock();
    expect(Date.now()).toBeGreaterThan(Date.UTC(2026, 0, 1) - 10 * 365 * 864e5);
    expect(Math.abs(Date.now() - new Date().getTime())).toBeLessThan(1000);
  });
});

describe('graph camera', () => {
  it('stores where the graph looks as a point in the graph, not in pixels', () => {
    // A pane of 1000x600 at zoom 2, shifted so that graph point (300, 100) is in the middle.
    expect(centerOf({ x: 500 - 600, y: 300 - 200, zoom: 2 }, { width: 1000, height: 600 })).toEqual({ cx: 300, cy: 100, zoom: 2 });
  });

  it('steers the graph when one is mounted, and says so when not', () => {
    const calls = [];
    graphView.api = null;
    expect(showView({ cx: 1, cy: 2, zoom: 1.5 })).toBe(false);
    graphView.api = { setCenter: (...a) => calls.push(a) };
    expect(showView({ cx: 1, cy: 2, zoom: 1.5 }, 140)).toBe(true);
    expect(calls).toEqual([[1, 2, { zoom: 1.5, duration: 140 }]]);
    graphView.api = null;
  });
});
