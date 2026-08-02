/**
 * Per-traject open-tabs: isolation, restore-on-entry and self-heal end-to-end.
 *
 * Two trajects (A owns `wet_alpha`, B owns `wet_beta`); a law GET for a law that
 * does not belong to the requested traject 404s, exactly as the real editor-api
 * does. That lets us prove:
 *   - opening a law in each traject keeps the two bars isolated;
 *   - a traject switch shows exactly ONE tab item carrying the destination
 *     traject's data-tab-key (a leftover/orphaned item - a "spooktab" - is
 *     itself an <nldd-document-tab-bar-item>, so the count assertion catches it
 *     without peering into shadow roots);
 *   - the last active article is restored and the URL replaced on entry;
 *   - a reload round-trips through localStorage (not just in-memory buckets);
 *   - polluted storage from an earlier build (traject A's law parked under
 *     traject B's key) heals: the tab 404s, disappears, the neutral state
 *     appears, and the key is cleaned.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg); the editor
 * mounts it but scenario execution is irrelevant here.
 */
import { test, expect } from '@playwright/test';
import {
  mockAuthedEditor,
  TEST_TRAJECT_REF,
  TEST_TRAJECT_B_REF,
} from './helpers.js';

const A = TEST_TRAJECT_REF;
const B = TEST_TRAJECT_B_REF;
const LAW_A = 'wet_alpha';
const LAW_B = 'wet_beta';

// Which laws each traject owns. A GET for a law NOT in the requested traject
// 404s - that is what a real cross-traject request does, and what the restore /
// self-heal logic keys on.
const MEMBERSHIP = {
  [A]: [LAW_A],
  [B]: [LAW_B],
};

const openTabsKey = (ref) => `regelrecht-open-tabs:${ref}`;

function lawYaml(id, name) {
  return `$id: ${id}
name: ${name}
regulatory_layer: WET
publication_date: '2024-12-20'
valid_from: '2025-01-01'
articles:
  - number: '1'
    text: 'Artikel 1 van ${name}.'
  - number: '2'
    text: 'Artikel 2 van ${name}.'
`;
}

const LAW_NAMES = { [LAW_A]: 'Wet Alpha', [LAW_B]: 'Wet Beta' };

async function mockCorpus(page) {
  await mockAuthedEditor(page);

  // Benign empties for the peripheral editor fetches so nothing errors while we
  // exercise the tabs. Registered first (lowest LIFO priority).
  await page.route('**/api/sources', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json', body: '[]' }),
  );
  await page.route('**/corpus/laws', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json', body: '[]' }),
  );
  await page.route('**/corpus/laws/*/versions', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json', body: '[]' }),
  );
  await page.route('**/corpus/laws/*/scenarios', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json', body: '[]' }),
  );
  await page.route('**/corpus/laws/*/scenarios/*', (r) =>
    r.fulfill({ status: 200, contentType: 'text/plain; charset=utf-8', body: '' }),
  );
  await page.route('**/corpus/laws/*/outputs', (r) =>
    r.fulfill({ status: 200, contentType: 'application/json', body: '[]' }),
  );

  // The law GET/PUT itself: traject-aware. Registered LAST so LIFO puts it
  // first; its regex only matches the bare `.../corpus/laws/{id}` tail, so the
  // sub-path handlers above still win their URLs via fallback.
  await page.route('**/trajects/*/corpus/laws/*', (route, request) => {
    const { pathname } = new URL(request.url());
    const match = pathname.match(/\/trajects\/([^/]+)\/corpus\/laws\/([^/]+)$/);
    if (!match) return route.fallback();
    if (request.method() === 'PUT') return route.fulfill({ status: 200, body: '' });
    const ref = decodeURIComponent(match[1]);
    const lawId = decodeURIComponent(match[2]);
    if (!(MEMBERSHIP[ref] || []).includes(lawId)) {
      return route.fulfill({ status: 404, body: `Law '${lawId}' not in traject` });
    }
    return route.fulfill({
      status: 200,
      contentType: 'text/yaml; charset=utf-8',
      body: lawYaml(lawId, LAW_NAMES[lawId]),
    });
  });
}

const tabItems = (page) => page.locator('nldd-document-tab-bar-item');
const neutralState = (page) =>
  page.getByText('Open een artikel vanuit de tabbalk of Home', { exact: false });

test.describe('Per-traject open tabs', () => {
  test.beforeEach(async ({ page }) => {
    // Deliberately NO clearOpenTabsStorage here: that helper is an
    // addInitScript that re-runs on EVERY navigation, which would wipe the tabs
    // this spec's reload/round-trip steps depend on. Playwright already gives
    // each test a fresh context (empty localStorage), so no clear is needed.
    await mockCorpus(page);
  });

  test('isolates, restores and self-heals across a traject switch', async ({ page }) => {
    test.setTimeout(60_000);

    // 1. Open wet in traject B.
    await page.goto(`/trajecten/${B}/editor/${LAW_B}/1`);
    await tabItems(page).first().waitFor({ timeout: 15_000 });
    await expect(tabItems(page)).toHaveCount(1);
    await expect(tabItems(page).first()).toHaveAttribute('data-tab-key', `${LAW_B}:1`);

    // 2. Open wet in traject A (a fresh load into A's editor).
    await page.goto(`/trajecten/${A}/editor/${LAW_A}/1`);
    await tabItems(page).first().waitFor({ timeout: 15_000 });
    await expect(tabItems(page)).toHaveCount(1);
    await expect(tabItems(page).first()).toHaveAttribute('data-tab-key', `${LAW_A}:1`);

    // 3. In-app switch to traject B via the traject menu: exactly one item, B's
    //    key (a spooktab would be a second item), and the URL restored to B's
    //    article by the restore-on-entry flow.
    // Elke viewport rendert zijn eigen TrajectMenu en verbergt de andere, dus
    // filter op zichtbaarheid in plaats van op een per-viewport id.
    await page.locator('[data-testid="traject-menu-trigger"]:visible').first().click();
    const trajectItemB = page.locator('nldd-menu-item[text="E2E Test Traject B"]').first();
    await trajectItemB.waitFor({ state: 'attached', timeout: 5_000 });
    // The traject menu-item is a radio that fires a custom `select` event (not a
    // plain click) - dispatch that to drive selectTraject -> in-app switch.
    await trajectItemB.evaluate((el) => el.dispatchEvent(new CustomEvent('select', { bubbles: true })));
    await page.waitForURL(`**/trajecten/${B}/editor/${LAW_B}/1`, { timeout: 15_000 });
    await expect(tabItems(page)).toHaveCount(1);
    await expect(tabItems(page).first()).toHaveAttribute('data-tab-key', `${LAW_B}:1`);

    // 4. Reload proves the localStorage round-trip (not just in-memory buckets):
    //    entering B's editor root restores B's remembered article.
    await page.goto(`/trajecten/${B}/editor`);
    await page.waitForURL(`**/trajecten/${B}/editor/${LAW_B}/1`, { timeout: 15_000 });
    await expect(tabItems(page)).toHaveCount(1);
    await expect(tabItems(page).first()).toHaveAttribute('data-tab-key', `${LAW_B}:1`);

    // 5. Back to A's editor root restores A's remembered article, isolated.
    await page.goto(`/trajecten/${A}/editor`);
    await page.waitForURL(`**/trajecten/${A}/editor/${LAW_A}/1`, { timeout: 15_000 });
    await expect(tabItems(page)).toHaveCount(1);
    await expect(tabItems(page).first()).toHaveAttribute('data-tab-key', `${LAW_A}:1`);
  });

  test('self-heals storage polluted with another traject\'s law', async ({ page }) => {
    test.setTimeout(60_000);

    // Poison B's storage with traject A's law (what an earlier build wrote), as
    // if it had been persisted under the wrong key.
    await page.addInitScript(
      ([key, tabs, activeKey, active]) => {
        try {
          window.localStorage.setItem(key, tabs);
          window.localStorage.setItem(activeKey, active);
        } catch { /* ignore */ }
      },
      [
        openTabsKey(B),
        JSON.stringify([{ lawId: LAW_A, articleNumber: '1' }]),
        `regelrecht-active-tab:${B}`,
        JSON.stringify({ lawId: LAW_A, articleNumber: '1' }),
      ],
    );

    // Entering B: the poisoned tab's law 404s in B, so it is pruned from the bar
    // and from localStorage, and the neutral state appears.
    await page.goto(`/trajecten/${B}/editor`);
    await expect(neutralState(page)).toBeVisible({ timeout: 15_000 });
    await expect(tabItems(page)).toHaveCount(0);

    // The polluted key is cleaned.
    const stored = await page.evaluate((k) => window.localStorage.getItem(k), openTabsKey(B));
    expect(JSON.parse(stored ?? '[]')).toEqual([]);
  });
});
