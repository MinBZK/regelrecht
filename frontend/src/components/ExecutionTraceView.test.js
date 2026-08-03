import { mount } from '@vue/test-utils';
import { describe, it, expect, beforeAll } from 'vitest';
import ExecutionTraceView from './ExecutionTraceView.vue';

// The real nldd-code-viewer isn't loaded under vitest, so stand in a minimal
// element with the one thing the directive needs: a shadow root carrying
// adoptedStyleSheets.
class StubCodeViewer extends HTMLElement {
  constructor() {
    super();
    this.attachShadow({ mode: 'open' });
  }
}

beforeAll(() => {
  if (!customElements.get('nldd-code-viewer')) {
    customElements.define('nldd-code-viewer', StubCodeViewer);
  }
});

const TRACE = 'wet_op_de_zorgtoeslag\n└──Result: heeft_recht = True';

const viewerIn = (w) => w.element.parentElement.querySelector('nldd-code-viewer');

const adoptedCss = (viewer) =>
  Array.from(viewer.shadowRoot.adoptedStyleSheets)
    .flatMap((s) => Array.from(s.cssRules).map((r) => r.cssText))
    .join('\n');

describe('ExecutionTraceView trace block', () => {
  it('does not wrap the trace, so the box-drawing columns stay aligned', () => {
    const w = mount(ExecutionTraceView, {
      props: { result: { outputs: {} }, traceText: TRACE },
      attachTo: document.body,
    });
    expect(viewerIn(w).hasAttribute('wrap')).toBe(false);
  });

  it('bounds the block so its scrollbars land inside the viewport', async () => {
    const w = mount(ExecutionTraceView, {
      props: { result: { outputs: {} }, traceText: TRACE },
      attachTo: document.body,
    });
    await w.vm.$nextTick();

    const viewer = viewerIn(w);
    const css = adoptedCss(viewer);
    // CodeMirror only takes a height through its own elements: a definite
    // height on .cm-editor and an overflow on .cm-scroller. Without both, the
    // block grows to its full content height and parks the horizontal
    // scrollbar thousands of pixels below the fold.
    expect(css).toMatch(/\.cm-editor[^}]*height:\s*100%/);
    expect(css).toMatch(/\.cm-scroller[^}]*overflow:\s*auto/);
    // .code-viewer is a flex child of the host; without min-height: 0 it
    // refuses to shrink below its content and the max-height does nothing.
    expect(css).toMatch(/\.code-viewer[^}]*min-height:\s*0/);
  });

  it('bounds the partial trace shown after a failed run too', async () => {
    const w = mount(ExecutionTraceView, {
      props: { error: 'boem', traceText: TRACE },
      attachTo: document.body,
    });
    await w.vm.$nextTick();

    expect(adoptedCss(viewerIn(w))).toMatch(/\.cm-scroller/);
  });
});
