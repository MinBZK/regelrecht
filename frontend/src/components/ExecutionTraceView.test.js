import { mount } from '@vue/test-utils';
import { describe, it, expect, beforeAll, afterEach } from 'vitest';
import ExecutionTraceView from './ExecutionTraceView.vue';

// Overflow metrics the stub viewer reports for the next mount. happy-dom does
// no layout, so the numbers a real browser would measure are supplied here
// instead. Defaults to a trace that overflows vertically but NOT horizontally —
// the exact shape the design system's own scroll-region logic misses, because
// it only ever measures the horizontal axis. The vertical numbers are the ones
// the e2e prints for the capped block on wet_op_de_zorgtoeslag art. 2.
const OVERFLOWS_VERTICALLY = { scrollHeight: 3114, clientHeight: 256, scrollWidth: 624, clientWidth: 624 };
const FITS = { scrollHeight: 43, clientHeight: 43, scrollWidth: 624, clientWidth: 624 };
let stubMetrics = OVERFLOWS_VERTICALLY;

// happy-dom reports 0 for every layout metric, so the ones the directive reads
// have to be planted on each scroller it will meet.
function makeScroller(metrics) {
  const scroller = document.createElement('div');
  scroller.className = 'cm-scroller';
  for (const [name, value] of Object.entries(metrics)) {
    Object.defineProperty(scroller, name, { value, configurable: true });
  }
  return scroller;
}

// The real nldd-code-viewer isn't loaded under vitest, so stand in a minimal
// element with the parts the directive works against: the `.code-viewer` render
// root, the `.cm-scroller` inside it, and Lit's `updateComplete`.
class StubCodeViewer extends HTMLElement {
  constructor() {
    super();
    const root = this.attachShadow({ mode: 'open' });
    const block = document.createElement('div');
    block.className = 'code-viewer';
    block.appendChild(makeScroller(stubMetrics));
    root.appendChild(block);
    this.updateComplete = Promise.resolve(true);
  }
}

beforeAll(() => {
  if (!customElements.get('nldd-code-viewer')) {
    customElements.define('nldd-code-viewer', StubCodeViewer);
  }
});

const TRACE = 'wet_op_de_zorgtoeslag\n└──Result: heeft_recht = True';

// This component has several root nodes, so `w.element` is the wrapper DIV that
// vue-test-utils puts around them — the query has to stay inside it. Reaching
// for `w.element.parentElement` lands on document.body, where every still-mounted
// wrapper's viewer lives, and the assertion silently reads the first test's.
const viewerIn = (w) => w.find('nldd-code-viewer').element;

const scrollerIn = (w) => viewerIn(w).shadowRoot.querySelector('.cm-scroller');

describe('ExecutionTraceView trace block', () => {
  const wrappers = [];

  // Every wrapper is attached to document.body, so leaving them mounted would
  // pile up nldd-code-viewer elements there and let one test observe another's.
  afterEach(() => {
    while (wrappers.length) wrappers.pop().unmount();
    document.body.innerHTML = '';
    stubMetrics = OVERFLOWS_VERTICALLY;
  });

  async function mountView(props) {
    const w = mount(ExecutionTraceView, { props, attachTo: document.body });
    wrappers.push(w);
    // The directive awaits customElements/updateComplete before it observes.
    await w.vm.$nextTick();
    await Promise.resolve();
    return w;
  }

  it('does not wrap the trace, so the box-drawing columns stay aligned', async () => {
    const w = await mountView({ result: { outputs: {} }, traceText: TRACE });
    expect(viewerIn(w).hasAttribute('wrap')).toBe(false);
  });

  it('makes the bounded scroll box reachable by keyboard and announceable', async () => {
    const w = await mountView({ result: { outputs: {} }, traceText: TRACE });

    // The height cap turns the block into a vertical scroll container. The
    // design system only marks its scroll region on HORIZONTAL overflow, so on
    // a trace like this one the region would otherwise be skipped when tabbing
    // (WCAG 2.1.1).
    const scroller = scrollerIn(w);
    expect(scroller.getAttribute('tabindex')).toBe('0');
    expect(scroller.getAttribute('role')).toBe('region');
    expect(scroller.getAttribute('aria-label')).toBe('Execution trace');
  });

  it('re-marks the region after the design system strips it', async () => {
    const w = await mountView({ result: { outputs: {} }, traceText: TRACE });
    const scroller = scrollerIn(w);

    // _updateScrollable() runs again on a rAF-debounced ResizeObserver, on
    // slot changes and on several property changes, and removes these whenever
    // the content isn't overflowing horizontally. A one-shot set is undone.
    scroller.removeAttribute('tabindex');
    scroller.removeAttribute('role');
    scroller.removeAttribute('aria-label');
    await Promise.resolve();

    expect(scroller.getAttribute('tabindex')).toBe('0');
    expect(scroller.getAttribute('role')).toBe('region');
    expect(scroller.getAttribute('aria-label')).toBe('Execution trace');
  });

  it('follows the scroll region to the scroller a re-mount builds', async () => {
    const w = await mountView({ result: { outputs: {} }, traceText: TRACE });
    const block = viewerIn(w).shadowRoot.querySelector('.code-viewer');

    // A detach/reattach makes the design system tear down its EditorView and
    // build a brand-new `.cm-scroller` inside the same `.code-viewer`. Marking
    // only the scroller we found at mount time would leave the live one bare,
    // which is why the observer watches childList/subtree and re-binds.
    block.querySelector('.cm-scroller').remove();
    const remounted = makeScroller(OVERFLOWS_VERTICALLY);
    block.appendChild(remounted);
    await Promise.resolve();

    expect(remounted.getAttribute('tabindex')).toBe('0');
    expect(remounted.getAttribute('role')).toBe('region');
    expect(remounted.getAttribute('aria-label')).toBe('Execution trace');
  });

  it('leaves a trace that fits alone, so it is not a bogus tab stop', async () => {
    stubMetrics = FITS;
    const w = await mountView({ result: { outputs: {} }, traceText: TRACE });

    expect(scrollerIn(w).hasAttribute('tabindex')).toBe(false);
    expect(scrollerIn(w).hasAttribute('role')).toBe(false);
  });

  it('marks the partial trace shown after a failed run too', async () => {
    const w = await mountView({ error: 'boem', traceText: TRACE });

    const scroller = scrollerIn(w);
    expect(scroller.getAttribute('tabindex')).toBe('0');
    expect(scroller.getAttribute('aria-label')).toBe('Execution trace');
  });

  it('stops observing once the view is unmounted', async () => {
    const w = await mountView({ result: { outputs: {} }, traceText: TRACE });
    const scroller = scrollerIn(w);

    w.unmount();
    wrappers.pop();
    scroller.removeAttribute('tabindex');
    await Promise.resolve();

    expect(scroller.hasAttribute('tabindex')).toBe(false);
  });
});
