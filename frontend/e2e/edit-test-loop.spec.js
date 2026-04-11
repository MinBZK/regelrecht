/**
 * Edit → re-execute loop end-to-end.
 *
 * Verifies the demo flow the editor was built for:
 *   1. Open zorgtoeslagwet in the editor.
 *   2. Observe the *Minderjarige heeft geen recht op zorgtoeslag* scenario is
 *      red (badge = ✗) — age check isn't in the law's machine_readable.
 *   3. Edit article 2's machine_readable via the middle-pane YAML editor to
 *      add a leeftijd input + an AGE-based condition to the existing AND.
 *   4. ScenarioBuilder auto-reexecutes against the edited YAML.
 *   5. Observe the scenario badge is now green (badge = ✓).
 *
 * This is the smoke-test for the propagation chain we just wired up:
 *   machineReadable edit → currentLawYaml computed → engine reload →
 *   ScenarioBuilder lawYaml prop → dependency reload → auto-execute.
 *
 * All corpus laws, the scenarios list, the scenario feature file, and the
 * PUT save endpoint are mocked from the on-disk corpus directory so the
 * spec doesn't need a running editor-api. This keeps it CI-friendly and
 * reproduces the real dependency graph.
 */
import { test, expect } from '@playwright/test';
import { readFileSync, readdirSync, statSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import yaml from 'js-yaml';

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS_ROOT = resolve(__dirname, '../../corpus/regulation/nl');

/**
 * Recursively walk a corpus directory and pick one YAML per `$id`,
 * preferring the latest publication date. Returns a map from law id →
 * { content: string, path: string }.
 *
 * NOTE: this picks by `publication_date` (lexicographic compare),
 * whereas the editor-api's `SourceMap::pick_best_version` selects by
 * `valid_from` against today's date. The two will agree as long as
 * each law in the test corpus has only one dated file (the case for
 * zorgtoeslagwet today). If a future test fixture introduces multiple
 * dated files for the same `$id`, align this with the server logic
 * to avoid the spec serving a different version than the editor would
 * see in production.
 */
function loadCorpus(rootDir) {
  const byId = new Map();

  function visit(dir) {
    for (const entry of readdirSync(dir)) {
      const full = resolve(dir, entry);
      if (statSync(full).isDirectory()) {
        visit(full);
      } else if (entry.endsWith('.yaml')) {
        const content = readFileSync(full, 'utf-8');
        const idMatch = content.match(/^\$id:\s*['"]?([^'"\n]+)['"]?$/m);
        if (!idMatch) continue;
        const lawId = idMatch[1].trim();
        const pubMatch = content.match(/^publication_date:\s*['"]?([^'"\n]+)['"]?$/m);
        const pubDate = pubMatch ? pubMatch[1].trim() : '';
        const existing = byId.get(lawId);
        if (!existing || pubDate > existing.pubDate) {
          byId.set(lawId, { content, path: full, pubDate });
        }
      }
    }
  }

  visit(rootDir);
  return byId;
}

/**
 * Find the scenario file for a law. Returns the raw `.feature` text or null.
 */
function loadScenario(lawPath, filename) {
  const scenariosDir = resolve(dirname(lawPath), 'scenarios');
  try {
    return readFileSync(resolve(scenariosDir, filename), 'utf-8');
  } catch {
    return null;
  }
}

/**
 * Set up route intercepts so the editor can fetch any law in the corpus
 * without a running editor-api. Also stubs the scenarios list/get and the
 * PUT save endpoint.
 */
async function mockCorpusApi(page, corpus, scenarioLaw, scenarioFile) {
  // GET /api/corpus/laws — list for dependency discovery.
  // Playwright runs route handlers in reverse registration order (LIFO), so
  // the more specific `/api/corpus/laws/*` routes below take precedence for
  // single-law and scenario paths; this bare-list handler only runs when no
  // later route claims the request.
  await page.route('**/api/corpus/laws*', (route, request) => {
    const url = new URL(request.url());
    if (url.pathname !== '/api/corpus/laws') {
      return route.fallback();
    }
    const entries = [...corpus.entries()].map(([law_id]) => ({
      law_id,
      name: null,
      source_id: 'local',
      source_name: 'Local Test Corpus',
    }));
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(entries),
    });
  });

  // PUT /api/corpus/laws/{law_id} — save endpoint. We no-op because the
  // frontend's useLaw.saveLaw() already updates rawYaml locally on success.
  // GET /api/corpus/laws/{law_id} — serve from corpus map
  await page.route('**/api/corpus/laws/*', (route, request) => {
    const url = new URL(request.url());
    const pathname = url.pathname;
    // Skip scenario sub-paths — separate handler below.
    if (pathname.includes('/scenarios')) {
      return route.fallback();
    }
    const lawId = decodeURIComponent(pathname.split('/').pop());
    if (request.method() === 'PUT') {
      return route.fulfill({ status: 200, body: '' });
    }
    const entry = corpus.get(lawId);
    if (!entry) {
      return route.fulfill({ status: 404, body: `Law '${lawId}' not found` });
    }
    return route.fulfill({
      status: 200,
      contentType: 'text/yaml; charset=utf-8',
      body: entry.content,
    });
  });

  // GET /api/corpus/laws/{law_id}/scenarios — list (only for the target law)
  await page.route('**/api/corpus/laws/*/scenarios', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(/\/api\/corpus\/laws\/([^/]+)\/scenarios$/);
    if (!match) return route.fallback();
    const lawId = decodeURIComponent(match[1]);
    if (lawId === scenarioLaw.id) {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([{ filename: scenarioLaw.scenarioFilename }]),
      });
    }
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: '[]',
    });
  });

  // GET /api/corpus/laws/{law_id}/scenarios/{filename}
  await page.route('**/api/corpus/laws/*/scenarios/*', (route, request) => {
    const url = new URL(request.url());
    const match = url.pathname.match(/\/api\/corpus\/laws\/([^/]+)\/scenarios\/([^/]+)$/);
    if (!match) return route.fallback();
    return route.fulfill({
      status: 200,
      contentType: 'text/plain; charset=utf-8',
      body: scenarioFile,
    });
  });

  // /api/sources — corpus source list (used by library page)
  await page.route('**/api/sources', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify([
        { id: 'local', name: 'Local Test Corpus', source_type: 'local', priority: 1, law_count: corpus.size },
      ]),
    }),
  );

  // /auth/* — OIDC is disabled in tests, return the disabled-state response
  // the frontend's useAuth expects.
  await page.route('**/auth/status', (route) =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ authenticated: false, oidc_configured: false }),
    }),
  );
}

test.describe('Edit → re-execute loop', () => {
  test.beforeEach(async ({ page }) => {
    // Clear localStorage tabs to avoid bleed-over between test runs.
    await page.addInitScript(() => {
      try {
        window.localStorage.removeItem('regelrecht-open-tabs');
      } catch { /* ignore */ }
    });
  });

  test('Minderjarige scenario goes red → green after adding age check', async ({ page }) => {
    const corpus = loadCorpus(CORPUS_ROOT);
    const zorgtoeslag = corpus.get('zorgtoeslagwet');
    expect(zorgtoeslag, 'zorgtoeslagwet must exist in the test corpus').toBeTruthy();

    const scenarioFilename = 'eligibility.feature';
    const scenarioText = loadScenario(zorgtoeslag.path, scenarioFilename);
    expect(scenarioText, 'eligibility.feature must exist').toBeTruthy();

    await mockCorpusApi(
      page,
      corpus,
      { id: 'zorgtoeslagwet', scenarioFilename },
      scenarioText,
    );

    // Navigate directly to article 2 via the query param — that's where
    // heeft_recht_op_zorgtoeslag lives and where we need to edit.
    await page.goto('/editor.html?law=zorgtoeslagwet&article=2');

    // Wait for the document tab bar to render — articles loaded.
    await page.waitForSelector('ndd-document-tab-bar-item', { timeout: 15_000 });

    const minorHeader = page
      .locator('.sb-accordion-header')
      .filter({ hasText: 'Minderjarige' });
    await expect(minorHeader).toBeVisible({ timeout: 30_000 });

    // Wait until the badge appears (either ✓ or ✗) — meaning execution
    // completed. The badge span has class sb-badge--pass or sb-badge--fail.
    await minorHeader
      .locator('.sb-badge--pass, .sb-badge--fail')
      .first()
      .waitFor({ timeout: 30_000 });

    // Initial state: scenario is failed (age check not in the law).
    await expect(minorHeader).toHaveClass(/sb-header--fail/);

    // Toggle the middle pane to YAML view. ndd-segmented-control-item is
    // a custom element whose click target lives in shadow DOM, so instead
    // of clicking we synthesize the change event the way EditorApp's
    // `onMiddlePaneChange` handler expects: it reads `event.target.value`
    // first, then falls back to `event.detail[0]`. The first
    // ndd-segmented-control in the page is the middle pane's form/yaml
    // toggle (the right pane's result/machine toggle comes after it).
    await page.locator('ndd-segmented-control').first().evaluate((el) => {
      el.value = 'yaml';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });
    // Wait for Vue to re-render the YAML pane.
    await page.waitForSelector('.editor-yaml-textarea', { timeout: 5000 });

    // Grab the current YAML (article 2's machine_readable), parse it,
    // surgically add the leeftijd input and the AND condition, and write
    // it back into the textarea.
    const textarea = page.locator('.editor-yaml-textarea');
    await expect(textarea).toBeVisible();

    const originalYaml = await textarea.inputValue();
    expect(originalYaml).toContain('heeft_recht_op_zorgtoeslag');
    const mr = yaml.load(originalYaml);

    // Inject leeftijd input (sourced from BRP with a literal peildatum).
    // BRP art 1.2 requires both bsn and peildatum; we use the scenario's
    // calculation date as a literal here.
    //
    // NOTE: a literal date is intentional for test isolation — the spec must
    // remain stable regardless of the real calculation date at CI time. The
    // canonical production pattern (see `kieswet`) references a parameter
    // like `peildatum: $verkiezingsdatum` so the date tracks the runtime
    // context; don't copy the literal form into corpus laws.
    mr.execution.input.push({
      name: 'leeftijd',
      type: 'number',
      source: {
        regulation: 'wet_basisregistratie_personen',
        output: 'leeftijd',
        parameters: {
          bsn: '$bsn',
          peildatum: '2025-01-01',
        },
      },
    });

    // Append the age condition to the heeft_recht_op_zorgtoeslag AND.
    // Assert the action is shaped the way we expect before mutating so a
    // future refactor (e.g. top-level op change) produces a legible error
    // instead of a bare `Cannot read properties of undefined` on .push.
    const heeftRecht = mr.execution.actions.find(
      (a) => a.output === 'heeft_recht_op_zorgtoeslag',
    );
    expect(heeftRecht, 'heeft_recht_op_zorgtoeslag action must exist').toBeTruthy();
    expect(
      heeftRecht.value?.operation,
      'heeft_recht action must be an AND at the top level',
    ).toBe('AND');
    expect(Array.isArray(heeftRecht.value?.conditions)).toBe(true);
    heeftRecht.value.conditions.push({
      operation: 'GREATER_THAN_OR_EQUAL',
      subject: '$leeftijd',
      value: 18,
    });

    const editedYaml = yaml.dump(mr, { lineWidth: 80, noRefs: true });

    // Fill the textarea by dispatching an input event; the editor's
    // onYamlInput handler parses the text and updates machineReadable.
    await textarea.evaluate((el, val) => {
      el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }, editedYaml);

    // Toggle back to the form view so the scenarios accordion mounts
    // again. The middle pane only shows one of the two views at a time;
    // remounting ScenarioBuilder kicks off its immediate `lawYaml` watch,
    // which reloads dependencies against the edited law and re-executes.
    await page.locator('ndd-segmented-control').first().evaluate((el) => {
      el.value = 'form';
      el.dispatchEvent(new Event('change', { bubbles: true }));
    });

    // Re-execution fires via the currentLawYaml → ScenarioBuilder lawYaml
    // prop chain. Allow the engine + dependency reload + scenario run to
    // complete.
    const minorHeaderAfter = page
      .locator('.sb-accordion-header')
      .filter({ hasText: 'Minderjarige' })
      .first();
    await expect(minorHeaderAfter).toHaveClass(/sb-header--pass/, { timeout: 60_000 });
  });
});
