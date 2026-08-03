/**
 * The execution trace renders as a bounded scroll box, not a 3000px column.
 *
 * The trace is a box-drawing tree, so it must not wrap (a wrapped continuation
 * restarts at column 0 and reads as depth 0). Without wrapping the block used to
 * grow to its full content height, which parked its horizontal scrollbar
 * thousands of pixels below the fold inside the sheet's own scroller — nobody
 * could find it (#1101).
 *
 * The unit tests can't prove this: happy-dom does no layout, so they can only
 * assert the strings we wrote, never that the block is actually short and
 * actually scrolls. That takes a real browser, so it lives here.
 *
 * Asserted against a real engine run of wet_op_de_zorgtoeslag:
 *   - the trace block is capped at 40% of the viewport, and
 *   - its bottom edge — where the horizontal scrollbar lives — lands *inside*
 *     the viewport, which is the property that actually matters and the one a
 *     height-only assertion does not prove: the block starts ~356px down, so a
 *     cap measured against the whole viewport can still end below the fold, and
 *   - its scroller really does scroll (scrollHeight > clientHeight, and
 *     scrollTop moves), so the content isn't merely clipped, and
 *   - the scroll region is reachable by keyboard and announced (WCAG 2.1.1).
 *
 * This law's trace overflows on BOTH axes, so the design system marks the
 * region itself here and only the label is ours. The case it misses — a trace
 * that overflows vertically but not horizontally — has no long-line-free
 * fixture in this corpus, so it is covered by the unit tests, which can dictate
 * the overflow metrics.
 *
 * All corpus endpoints are mocked from the on-disk corpus; the engine is the
 * real WASM build running client-side (frontend/public/wasm/pkg, via
 * `just wasm-build`).
 */
import { test, expect } from '@playwright/test';
import { loadCorpus, loadScenario, mockCorpusApi } from './helpers-corpus.js';
import { gotoEditor, expectScenarioResult, clearOpenTabsStorage, openSheet } from './helpers.js';

const MINOR = 'Minderjarige heeft geen recht';

test.describe('Execution trace scroll box', () => {
  test.beforeEach(async ({ page }) => {
    await clearOpenTabsStorage(page);
  });

  test('the trace is a bounded scroll box, reachable by keyboard', async ({ page }) => {
    // Full-corpus dependency loading plus every scenario auto-executing needs
    // well over the default 30s budget.
    test.setTimeout(180_000);

    const corpus = loadCorpus();
    const zorgtoeslag = corpus.get('wet_op_de_zorgtoeslag');
    expect(zorgtoeslag, 'wet_op_de_zorgtoeslag must exist in the test corpus').toBeTruthy();

    const scenarioFilename = 'eligibility.feature';
    const scenarioText = loadScenario(zorgtoeslag.path, scenarioFilename);
    expect(scenarioText, 'eligibility.feature must exist').toBeTruthy();

    await mockCorpusApi(
      page,
      corpus,
      { id: 'wet_op_de_zorgtoeslag', scenarioFilename },
      scenarioText,
    );

    await gotoEditor(page, 'wet_op_de_zorgtoeslag', '2');

    // Leaves the result sheet open on the executed scenario.
    await expectScenarioResult(page, MINOR, 'Mislukt');

    const trace = openSheet(page).locator('nldd-code-viewer.etv-trace');
    await trace.waitFor({ state: 'visible', timeout: 10_000 });

    // Wrapping is what destroyed the tree alignment; it must stay off.
    expect(await trace.evaluate((el) => el.hasAttribute('wrap'))).toBe(false);

    const box = await trace.evaluate(async (el) => {
      const scroller = el.shadowRoot.querySelector('.cm-scroller');
      scroller.scrollTop = 99_999;
      await new Promise(requestAnimationFrame);
      const scrolledTo = scroller.scrollTop;
      scroller.scrollTop = 0;
      const rect = el.getBoundingClientRect();
      return {
        blockTop: rect.top,
        blockHeight: rect.height,
        blockBottom: rect.bottom,
        viewportHeight: window.innerHeight,
        scrollHeight: scroller.scrollHeight,
        clientHeight: scroller.clientHeight,
        scrolledTo,
        tabindex: scroller.getAttribute('tabindex'),
        role: scroller.getAttribute('role'),
        ariaLabel: scroller.getAttribute('aria-label'),
      };
    });
    console.log('trace scroll box:', JSON.stringify(box));

    // The trace this scenario produces has to be long enough to overflow,
    // otherwise the rest of the assertions prove nothing.
    expect(
      box.scrollHeight,
      'the trace must overflow its box, or this spec guards nothing',
    ).toBeGreaterThan(box.clientHeight);

    // The regression: the block must be bounded instead of growing to its full
    // content height.
    expect(box.blockHeight).toBeLessThanOrEqual(box.viewportHeight * 0.4 + 1);

    // …and bounded is not enough on its own. The block starts partway down the
    // sheet (356px on this fixture), so a cap measured against the whole
    // viewport can still push the bottom edge — and with it the horizontal
    // scrollbar — below the fold. This is the assertion that pins the cap to
    // what actually fits: at max-height 60vh the block is 432px, well inside
    // "60% of the viewport", yet it ends at 788px on a 720px screen. Measured
    // here at 40vh: top 356, height 288, bottom 644.
    expect(
      box.blockBottom,
      `the block's bottom edge (and its horizontal scrollbar) must be on screen; `
      + `top ${box.blockTop}, height ${box.blockHeight}`,
    ).toBeLessThanOrEqual(box.viewportHeight);

    // Bounded AND scrollable - not merely clipped.
    expect(box.scrolledTo).toBeGreaterThan(0);

    // WCAG 2.1.1: the scroll region must be in the tab order and announced.
    expect(box.tabindex).toBe('0');
    expect(box.role).toBe('region');
    expect(box.ariaLabel).toBe('Execution trace');
  });
});
