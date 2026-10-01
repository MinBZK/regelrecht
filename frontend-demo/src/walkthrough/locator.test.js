import { afterEach, describe, expect, it } from 'vitest';
import { describe as describeEl, loosen, resolve, signature } from './locator.js';
import { installClock, uninstallClock } from './clock.js';

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
