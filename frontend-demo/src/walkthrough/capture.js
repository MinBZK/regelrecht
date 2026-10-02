/**
 * What the recorder writes down while the presenter works: the actions the
 * player will do again in the live demo.
 *
 * Only trusted events (a person, not a script) and only outside the recorder
 * panel and the deck: slide changes are logged by the deck itself, as an
 * index, because "next slide" is what the player has to do and not "click
 * the arrow at x, y".
 *
 * - click: the element acted on (`locator.describe`) and where in it;
 * - input: the field and its value after every keystroke, so text appears in
 *   the replay at the pace it was typed;
 * - change: a select, checkbox or radio;
 * - key: Enter and Escape, which submit and close things;
 * - scroll: a scrolled element and its position, at most a few times a second;
 * - viewport: where the graph looks (graphBridge.js), while it is dragged or
 *   zoomed, ten times a second and once more when it comes to rest.
 *
 * What is typed is recorded: the replay has to type it again. It is demo
 * input in a demo with fictitious personas, said aloud in the same take.
 */
import { actionTarget, describe, offsetIn } from './locator.js';
import { graphView } from './graphBridge.js';

const IGNORE = '.recorder, .deck';
const KEYS = new Set(['Enter', 'Escape']);

function ignored(event) {
  return (event.composedPath?.() ?? []).some((n) => n?.matches?.(IGNORE));
}

function field(event) {
  const el = event.composedPath?.()[0];
  return el?.nodeType === 1 && /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName) ? el : null;
}

/**
 * Start listening; `log(event)` receives each action. Returns the function
 * that stops listening.
 */
export function captureActions(log) {
  const lastScroll = new WeakMap();

  function onClick(e) {
    if (!e.isTrusted || ignored(e)) return;
    const el = actionTarget(e);
    if (!el) return;
    log({ type: 'click', target: describe(el), ...offsetIn(el, e.clientX, e.clientY) });
  }

  function onInput(e) {
    if (!e.isTrusted || ignored(e)) return;
    const el = field(e);
    if (!el || el.type === 'checkbox' || el.type === 'radio') return;
    log({ type: 'input', target: describe(el), value: el.value });
  }

  function onChange(e) {
    if (!e.isTrusted || ignored(e)) return;
    const el = field(e);
    if (!el) return;
    if (el.type === 'checkbox' || el.type === 'radio') log({ type: 'change', target: describe(el), checked: el.checked });
    else if (el.tagName === 'SELECT') log({ type: 'change', target: describe(el), value: el.value });
  }

  function onKey(e) {
    if (!e.isTrusted || !KEYS.has(e.key) || ignored(e)) return;
    const el = e.composedPath?.()[0];
    log({ type: 'key', key: e.key, target: el?.nodeType === 1 && el !== document.body ? describe(el) : null });
  }

  function onScroll(e) {
    if (!e.isTrusted) return;
    const el = e.target === document ? document.scrollingElement : e.target;
    if (!el || el.nodeType !== 1 || el.closest?.(IGNORE)) return;
    const now = performance.now();
    if (now - (lastScroll.get(el) ?? 0) < 150) return;
    lastScroll.set(el, now);
    log({ type: 'scroll', target: el === document.scrollingElement ? null : describe(el), top: Math.round(el.scrollTop), left: Math.round(el.scrollLeft) });
  }

  // The graph's camera: throttled, with the resting position always logged,
  // so a replay ends exactly where the presenter stopped.
  let lastView = 0;
  let pending = null;
  graphView.onView = (view) => {
    clearTimeout(pending);
    const now = performance.now();
    if (now - lastView >= 100) {
      lastView = now;
      log({ type: 'viewport', ...view });
    } else {
      pending = setTimeout(() => {
        lastView = performance.now();
        log({ type: 'viewport', ...view });
      }, 150);
    }
  };

  document.addEventListener('click', onClick, true);
  document.addEventListener('input', onInput, true);
  document.addEventListener('change', onChange, true);
  document.addEventListener('keydown', onKey, true);
  document.addEventListener('scroll', onScroll, true);
  return () => {
    clearTimeout(pending);
    graphView.onView = null;
    document.removeEventListener('click', onClick, true);
    document.removeEventListener('input', onInput, true);
    document.removeEventListener('change', onChange, true);
    document.removeEventListener('keydown', onKey, true);
    document.removeEventListener('scroll', onScroll, true);
  };
}
